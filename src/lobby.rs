use crate::{
    ErrorMessage, ServerMessage, SessionId,
    game::{
        GameInfo, GameMessage, GameState, LobbySettings, StartMatchError,
        r#match::MatchTerminationType, round::RoundState,
    },
};
use rand::{RngExt, seq::IndexedRandom};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};
use tokio::{
    select,
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    time::sleep,
};
use tokio_util::task::JoinMap;

pub type ConnectionTx = UnboundedSender<ServerMessage>;

pub async fn lobby_task(
    lobby_code: LobbyCode,
    host_id: SessionId,
    host_tx: ConnectionTx,
    mut lobby_rx: UnboundedReceiver<InternalLobbyMessage>,
    // to send messages within itself with timed events
    lobby_tx: UnboundedSender<InternalLobbyMessage>,
    // to remove the session lobbymessage forwarding into this lobby, if a player leaves the lobby for any reason
    remove_session_tx: UnboundedSender<SessionId>,
) {
    let mut lobby = Lobby::new(lobby_code, host_id, lobby_tx.clone());
    lobby_tx
        .send(InternalLobbyMessage::SessionConnected {
            session_id: host_id,
            connection_tx: host_tx,
        })
        .inspect_err(|e| tracing::error!(error = %e))
        .ok();

    // set up a timer on disconnect message
    let disconnect_duration = Duration::from_secs(15);
    let mut disconnect_timers = JoinMap::new();

    'lobby_runtime: loop {
        select! {
            Some((session_id, _result)) = disconnect_timers.join_next() => {
                if lobby.session_left(session_id, LeftLobbyReason::Disconnected, &remove_session_tx) {
                    break 'lobby_runtime;
                };
            }
            Some(message) = lobby_rx.recv() => {
                match message {
                    InternalLobbyMessage::LobbyMessage { session_id, message } => {
                        if lobby.handle_lobby_message(session_id, message, &remove_session_tx) {
                            break 'lobby_runtime;
                        }
                    },
                    InternalLobbyMessage::AutoMessage(message) => {
                        lobby.handle_auto_message(message);
                    },
                    InternalLobbyMessage::SessionConnected { session_id, connection_tx } => {
                        disconnect_timers.abort(&session_id);
                        lobby.session_connected(session_id, connection_tx);
                    },
                    InternalLobbyMessage::SessionOffline(session_id) => {
                        disconnect_timers.spawn(session_id, async move { sleep(disconnect_duration).await; });
                        lobby.session_offline(session_id);
                    },
                }
            }
        };
    }
}

#[derive(Debug)]
pub struct Lobby {
    pub lobby_code: LobbyCode,
    pub players: LobbyPlayers,
    pub broadcaster: LobbyBroadcaster,
    lobby_tx: UnboundedSender<InternalLobbyMessage>,

    // pub auto_message_handler: JoinHandle<()>,
    pub settings: LobbySettings,
    pub game_state: Option<GameState>,
}

impl Lobby {
    pub fn new(
        lobby_code: LobbyCode,
        host_session: SessionId,
        lobby_tx: UnboundedSender<InternalLobbyMessage>,
    ) -> Self {
        let players = LobbyPlayers::new(host_session);
        let broadcaster = LobbyBroadcaster::new();
        Self {
            lobby_code,
            players,
            broadcaster,
            lobby_tx,
            settings: LobbySettings::default(),
            game_state: None,
        }
    }

    // could mean either player reconnected or joined
    pub fn session_connected(&mut self, session_id: SessionId, connection_tx: ConnectionTx) {
        match self.players.add_player(session_id) {
            // player just joined
            Ok(player_id) => {
                // send to existing players, excluding the new player
                self.broadcaster
                    .broadcast(ServerMessage::PlayerJoined(player_id));
                self.broadcaster
                    .add_player_tx(player_id, connection_tx.clone());
                self.broadcaster.send_to_player(
                    &player_id,
                    ServerMessage::ConnectedToLobby(self.info(player_id)),
                );
            }
            // player already exists and reconnected (there is no way to reach this state by joining on two different devices simultaneously,
            // as they would've been hit with ErrorMessage::AlreadyInLobby at join attempt)
            Err(player_id) => {
                // send to existing players, excluding the new player
                self.broadcaster
                    .broadcast(ServerMessage::PlayerBackOnline(player_id));
                self.broadcaster.add_player_tx(player_id, connection_tx);
                self.broadcaster.send_to_player(
                    &player_id,
                    ServerMessage::ConnectedToLobby(self.info(player_id)),
                );
                return;
            }
        }
    }

    // returns true if the lobby should end, where it should have sent all close messages before that
    fn handle_lobby_message(
        &mut self,
        session_id: SessionId,
        message: LobbyMessage,
        remove_session_tx: &UnboundedSender<SessionId>,
    ) -> bool {
        let player_id = *self.players.get_player_id(&session_id).unwrap();
        match message {
            LobbyMessage::LeaveLobby => {
                if self.session_left(session_id, LeftLobbyReason::Left, remove_session_tx) {
                    return true;
                }
            }
            LobbyMessage::FetchLobbyInfo => {
                self.broadcaster.send_to_player(
                    &player_id,
                    ServerMessage::ConnectedToLobby(self.info(player_id)),
                );
            }
            LobbyMessage::GameMessage(game_message) => match self.game_state {
                Some(ref mut game_state) => {
                    game_state.handle_message(player_id, game_message, &self.broadcaster);
                }
                None => {
                    self.broadcaster
                        .send_to_player(&player_id, ServerMessage::Error(ErrorMessage::NotInGame));
                }
            },
            LobbyMessage::HostMessage(host_message) => {
                if player_id != self.players.host() {
                    self.broadcaster.send_to_player(
                        &player_id,
                        ServerMessage::Error(ErrorMessage::InsufficientPermissions),
                    );
                    return false;
                }
                //
                match host_message {
                    HostMessage::StartGame => {
                        if self.game_state.is_some() {
                            self.broadcaster.send_to_player(
                                &player_id,
                                ServerMessage::Error(ErrorMessage::AlreadyInGame),
                            );
                            return false;
                        }
                        let game = GameState::new(&self.settings, self.lobby_tx.clone());
                        self.broadcaster
                            .broadcast(ServerMessage::GameStarted(game.info()));
                        self.game_state = Some(game);
                        return false;
                    }
                    HostMessage::StartMatch => {
                        let Some(ref mut game_state) = self.game_state else {
                            self.broadcaster.send_to_player(
                                &player_id,
                                ServerMessage::Error(ErrorMessage::NotInGame),
                            );
                            return false;
                        };
                        if let Err(e) = game_state.start_match(&self.players, &self.broadcaster) {
                            match e {
                                StartMatchError::AlreadyOngoingMatch => {
                                    self.broadcaster.send_to_player(
                                        &player_id,
                                        ServerMessage::Error(ErrorMessage::AlreadyInMatch),
                                    );
                                }
                                StartMatchError::TooHighNCardsPerPlayer => {
                                    self.broadcaster.send_to_player(
                                        &player_id,
                                        ServerMessage::Error(ErrorMessage::Other("Failed to start round, settings.n_cards_per_player too high for card pool".to_string()))
                                    );
                                }
                                StartMatchError::TooFewPlayers => {
                                    self.broadcaster.send_to_player(
                                        &player_id,
                                        ServerMessage::Error(ErrorMessage::Other("Failed to start round, too few players in lobby (requires a minimum of 3)".to_string()))
                                    );
                                }
                            }
                            return false;
                        };
                        return false;
                    }
                    HostMessage::StartRound => {
                        let Some(ref mut game_state) = self.game_state else {
                            self.broadcaster.send_to_player(
                                &player_id,
                                ServerMessage::Error(ErrorMessage::NotInGame),
                            );
                            return false;
                        };
                        let Some(ref mut current_match) = game_state.current_match else {
                            self.broadcaster.send_to_player(
                                &player_id,
                                ServerMessage::Error(ErrorMessage::NotInMatch),
                            );
                            return false;
                        };
                        if current_match.current_round.is_some() {
                            self.broadcaster.send_to_player(
                                &player_id,
                                ServerMessage::Error(ErrorMessage::AlreadyInRound),
                            );
                            return false;
                        }
                        let starting_player = match current_match.previous_rounds.last() {
                            Some(round_state) => *round_state
                                .round_termination
                                .as_ref()
                                .unwrap()
                                .get_loser()
                                .unwrap(),
                            None => {
                                match game_state.previous_matches.iter().rev().find_map(|state| {
                                    if let Some(MatchTerminationType::PlayerWonMatch(player)) =
                                        state.match_termination
                                    {
                                        Some(player)
                                    } else {
                                        None
                                    }
                                }) {
                                    Some(winner) => winner,
                                    None => *current_match
                                        .players
                                        .get_player_vec()
                                        .choose(&mut rand::rng())
                                        .unwrap(),
                                }
                            }
                        };
                        let current_round = RoundState::new(starting_player);
                        self.broadcaster
                            .broadcast(ServerMessage::RoundStarted(current_round.info()));
                        current_match.current_round = Some(current_round);
                        return false;
                    }
                    HostMessage::AddNewRule => {
                        let Some(ref mut game_state) = self.game_state else {
                            self.broadcaster.send_to_player(
                                &player_id,
                                ServerMessage::Error(ErrorMessage::NotInGame),
                            );
                            return false;
                        };
                        if let Ok(rule_info) = game_state.rule_manager.add_rule() {
                            self.broadcaster
                                .broadcast(ServerMessage::RuleAdded(rule_info));
                        }
                        return false;
                    }
                    HostMessage::RemoveRule(id) => {
                        let Some(ref mut game_state) = self.game_state else {
                            self.broadcaster.send_to_player(
                                &player_id,
                                ServerMessage::Error(ErrorMessage::NotInGame),
                            );
                            return false;
                        };
                        if game_state.rule_manager.remove_rule(&id) {
                            self.broadcaster.broadcast(ServerMessage::RuleRemoved(id));
                        }
                        return false;
                    }
                    HostMessage::TransferHost(new_host) => {
                        if self.players.set_host(new_host).is_some() {
                            self.broadcaster
                                .broadcast(ServerMessage::HostChanged(self.players.host()));
                        } else {
                            self.broadcaster.send_to_player(
                                &player_id,
                                ServerMessage::Error(ErrorMessage::Other(
                                    "Invalid host transfer recipient".to_string(),
                                )),
                            );
                        }
                        return false;
                    }
                    HostMessage::SetSettings(lobby_settings) => {
                        self.settings = lobby_settings.clone();
                        self.broadcaster
                            .broadcast(ServerMessage::LobbySettingsUpdated(lobby_settings.clone()));
                        if let Some(ref mut game_state) = self.game_state {
                            if let Some(new_active_game_settings) =
                                game_state.lobby_settings_updated(lobby_settings)
                            {
                                self.broadcaster.broadcast(
                                    ServerMessage::ActiveGameSettingsUpdated(
                                        new_active_game_settings,
                                    ),
                                );
                            }
                        }
                    }
                    // HostMessage::AddBot => {
                    //     // TODO, not implemented
                    // }
                    HostMessage::KickPlayer(_player_id) => {
                        // check if player in game or match, if so call
                        // self.game_state.remove_player or the corresponding match variant
                        // TODO not implemented
                        return false;
                    }
                }
            }
        }
        false
    }

    fn handle_auto_message(&mut self, message: AutoMessage) {
        if let Some(ref mut game_state) = self.game_state {
            game_state.handle_auto_message(message, &self.broadcaster);
        }
    }

    pub fn session_offline(&mut self, session_id: SessionId) {
        if let Some(player_id) = self.players.get_player_id(&session_id) {
            self.broadcaster.remove_player_tx(&player_id);
            self.broadcaster
                .broadcast(ServerMessage::PlayerOffline(*player_id));
        } else {
            tracing::error!("somehow session offline is called with an invalid session: {self:?}");
        }
    }

    /// returns true if the lobby should end because all players left
    ///
    /// will also remove the lobby message forwarding via remove_session_tx
    pub fn session_left(
        &mut self,
        session_id: SessionId,
        reason: LeftLobbyReason,
        remove_session_tx: &UnboundedSender<SessionId>,
    ) -> bool {
        remove_session_tx
            .send(session_id)
            .inspect_err(|e| tracing::error!(error = %e))
            .ok();
        match self
            .players
            .remove_by_session_id(&session_id, &self.broadcaster)
        {
            Ok(player_id) => {
                self.broadcaster.remove_player_tx(&player_id);
                self.broadcaster
                    .broadcast(ServerMessage::PlayerLeft { player_id, reason });
                if let Some(ref mut game_state) = self.game_state {
                    game_state.handle_player_left(&player_id, &self.broadcaster);
                }
                false
            }
            Err(RemovePlayerError::SessionDoesNotExist) => {
                tracing::error!(
                    "removing player failed, session does not exist in player map: {session_id}"
                );
                false
            }
            Err(RemovePlayerError::NoPlayersLeft) => true,
        }
    }

    fn info(&self, player: PlayerId) -> LobbyInfo {
        LobbyInfo {
            you: player,
            lobby_code: self.lobby_code.clone(),
            players: self.players.players().copied().collect(),
            host: self.players.host(),
            settings: self.settings.clone(),
            current_game: self.game_state.as_ref().map(|state| state.info()),
        }
    }
}

// this is fetched on connecting to a lobby
#[derive(Clone, Serialize)]
pub struct LobbyInfo {
    you: PlayerId,
    lobby_code: LobbyCode,
    players: Vec<PlayerId>,
    host: PlayerId,
    settings: LobbySettings,
    current_game: Option<GameInfo>,
}

pub enum LobbyMessage {
    LeaveLobby,
    FetchLobbyInfo,
    GameMessage(GameMessage),
    HostMessage(HostMessage),
}

pub enum HostMessage {
    StartGame,
    StartMatch,
    StartRound,
    AddNewRule,
    RemoveRule(usize),
    TransferHost(PlayerId),
    // not implemented
    // AddBot,
    KickPlayer(PlayerId),
    SetSettings(LobbySettings),
}

pub enum AutoMessage {
    ActionTimeout,
    AutoStartMatch,
    AutoStartRound,
}
pub enum InternalLobbyMessage {
    SessionConnected {
        session_id: SessionId,
        connection_tx: ConnectionTx,
    },
    LobbyMessage {
        session_id: SessionId,
        message: LobbyMessage,
    },
    AutoMessage(AutoMessage),
    /// warns if the player's websocket session disconnected
    SessionOffline(SessionId),
}

#[derive(Clone, Serialize)]
pub enum LeftLobbyReason {
    Left,
    KickedByHost,
    Disconnected,
}

/// the public incremental id everyone in the lobby knows
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PlayerId(usize);

impl From<usize> for PlayerId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct LobbyCode(String);

impl LobbyCode {
    pub fn new() -> Self {
        const SYMBOLS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        let mut code = String::with_capacity(6);
        let mut rng = rand::rng();
        for _ in 0..6 {
            let index = rng.random_range(0..SYMBOLS.len());
            code.push(SYMBOLS[index] as char);
        }
        Self(code)
    }
}

/// stores all online players and their connection
#[derive(Debug, Default)]
pub struct LobbyBroadcaster {
    player_tx: HashMap<PlayerId, ConnectionTx>,
}

impl LobbyBroadcaster {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn send_to_player(&self, player_id: &PlayerId, message: ServerMessage) {
        self.player_tx
            .get(player_id)
            .unwrap()
            .send(message)
            .inspect_err(|e| tracing::error!(error = %e))
            .ok();
    }

    /// send a server message to all online players in the lobby
    pub fn broadcast(&self, message: ServerMessage) {
        for connection_tx in self.player_tx.values() {
            connection_tx
                .send(message.clone())
                .inspect_err(|e| tracing::error!(error = %e))
                .ok();
        }
    }

    /// get a player's tx
    pub fn get_player_tx(&self, player_id: &PlayerId) -> Option<&ConnectionTx> {
        self.player_tx.get(player_id)
    }
    /// if a player disconnects, remove the dead tx
    pub fn remove_player_tx(&mut self, player_id: &PlayerId) {
        self.player_tx.remove(player_id);
    }
    /// if a player reconnects, add back their tx
    pub fn add_player_tx(&mut self, player_id: PlayerId, connection_tx: ConnectionTx) {
        self.player_tx.insert(player_id, connection_tx);
    }
}
#[derive(Debug)]
pub struct LobbyPlayers {
    host: PlayerId,
    /// next public player id, for incremental assignment
    next_player_id: usize,

    session_player: HashMap<SessionId, PlayerId>,
    player_session: BTreeMap<PlayerId, SessionId>,
}
#[derive(Debug)]
pub enum LobbyPlayersInsertError {
    PlayerAlreadyExists,
    SessionAlreadyExists,
    PlayerAndSessionAlreadyExists,
}

impl LobbyPlayers {
    pub fn new(host_session: SessionId) -> Self {
        let host = PlayerId(0);
        Self {
            host,
            next_player_id: 1,
            session_player: HashMap::from([(host_session, host)]),
            player_session: BTreeMap::from([(host, host_session)]),
        }
    }
    pub fn len(&self) -> usize {
        self.session_player.len()
    }
    /// returns Err if the session already exists, and its corresponding player_id
    pub fn add_player(&mut self, session_id: SessionId) -> Result<PlayerId, PlayerId> {
        if let Some(player) = self.session_player.get(&session_id) {
            return Err(*player);
        }
        let player_id = PlayerId(self.next_player_id);
        self.next_player_id += 1;
        self.session_player.insert(session_id, player_id);
        self.player_session.insert(player_id, session_id);
        Ok(player_id)
    }

    fn host(&self) -> PlayerId {
        self.host
    }

    /// returns Some(old_host) if the host was replaced, else None
    ///
    /// The host will not be replaced if new_host isn't a valid player that exists
    fn set_host(&mut self, new_host: PlayerId) -> Option<PlayerId> {
        if self.host == new_host || !self.player_session.contains_key(&new_host) {
            None
        } else {
            Some(std::mem::replace(&mut self.host, new_host))
        }
    }

    /// returns an iterator through all players in order of their PlayerId
    pub fn players(&self) -> impl Iterator<Item = &PlayerId> {
        self.player_session.keys()
    }
    pub fn get_player_id(&self, session_id: &SessionId) -> Option<&PlayerId> {
        self.session_player.get(session_id)
    }
    pub fn session_tx_values(&self) -> impl Iterator<Item = &SessionId> {
        self.player_session.values()
    }
    // pub fn remove_by_player_id(&mut self, player_id: &PlayerId) -> Option<SessionId> {
    //     let session_id = self.player_session.remove(player_id)?;
    //     self.session_player.remove(&session_id);
    //     Some(session_id)
    // }

    /// return Err if it didn't remove as this was the last player and now all players are gone,
    /// and there's noone to set as host
    pub fn remove_by_session_id(
        &mut self,
        session_id: &SessionId,
        broadcaster: &LobbyBroadcaster,
    ) -> Result<PlayerId, RemovePlayerError> {
        if self.player_session.len() <= 1 {
            return Err(RemovePlayerError::NoPlayersLeft);
        }
        let Some(player_id) = self.session_player.remove(session_id) else {
            return Err(RemovePlayerError::SessionDoesNotExist);
        };
        self.player_session.remove(&player_id);
        if player_id == self.host {
            let new_host = self.player_session.keys().nth(0).unwrap();
            self.host = *new_host;
            broadcaster.broadcast(ServerMessage::HostChanged(self.host));
        }
        Ok(player_id)
    }

    pub fn contains_player(&self, player: &PlayerId) -> bool {
        self.player_session.contains_key(player)
    }
}

pub enum RemovePlayerError {
    NoPlayersLeft,
    SessionDoesNotExist,
}
