use crate::{
    ErrorMessage, ServerMessage, SessionId,
    game::{
        GameInfo, GameMessage, GameState, LobbySettings, r#match::MatchState, round::RoundState,
    },
};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    pin::Pin,
    time::Duration,
};
use tokio::{
    select,
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    time::Sleep,
};

pub type ConnectionTx = UnboundedSender<ServerMessage>;

pub async fn lobby_task(
    lobby_code: LobbyCode,
    host_id: SessionId,
    host_tx: ConnectionTx,
    mut lobby_rx: UnboundedReceiver<InternalLobbyMessage>,
    // to send messages within itself with timed events
    lobby_tx: UnboundedSender<InternalLobbyMessage>,
    remove_session_tx: UnboundedSender<SessionId>,
) {
    let mut lobby = Lobby::new(lobby_code, host_id, host_tx, lobby_tx);

    // set up a timer on disconnect message
    let disconnect_duration = Duration::from_secs(15);
    let mut disconnect_timer: Option<(SessionId, Pin<Box<Sleep>>)> = None;

    'lobby_runtime: loop {
        select! {
            _ = async { disconnect_timer.as_mut().unwrap().1.as_mut().await }, if disconnect_timer.is_some() => {
                let session_id = disconnect_timer.as_ref().unwrap().0;

                remove_session_tx.send(session_id).inspect_err(|e| tracing::error!(error = %e)).ok();
                if lobby.session_left(session_id, LeftLobbyReason::Disconnected) {
                    break 'lobby_runtime;
                };
            }
            Some(message) = lobby_rx.recv() => {
                match message {
                    InternalLobbyMessage::LobbyMessage { session_id, message } => {
                        if lobby.handle_lobby_message(session_id, message) {
                            break 'lobby_runtime;
                        }
                    },
                    InternalLobbyMessage::AutoMessage(message) => {
                        lobby.handle_auto_message(message);
                    },
                    InternalLobbyMessage::PlayerConnected { session_id, connection_tx } => {
                        lobby.session_connected(session_id, connection_tx);
                    },
                    InternalLobbyMessage::PlayerOffline(session_id) => {
                        disconnect_timer = Some((session_id, Box::pin(tokio::time::sleep(disconnect_duration))));
                        lobby.session_offline(session_id);
                    },
                    InternalLobbyMessage::PlayerLeft(session_id) => {

                        remove_session_tx.send(session_id).inspect_err(|e| tracing::error!(error = %e)).ok();
                        if lobby.session_left(session_id, LeftLobbyReason::Left) {
                            break 'lobby_runtime;
                        }
                    },
                }
            }
        };
    }
}

pub struct Lobby {
    pub lobby_code: LobbyCode,
    pub player_map: LobbyPlayers,
    pub broadcaster: LobbyBroadcaster,
    lobby_tx: UnboundedSender<InternalLobbyMessage>,
    pub host: PlayerId,

    /// next public player id, for incremental assignment
    pub next_player_id: usize,

    pub settings: LobbySettings,
    pub game_state: Option<GameState>,
}

impl Lobby {
    pub fn new(
        lobby_code: LobbyCode,
        host_session: SessionId,
        host_tx: ConnectionTx,
        lobby_tx: UnboundedSender<InternalLobbyMessage>,
    ) -> Self {
        let host_id = PlayerId::from(0);
        let mut player_map = LobbyPlayers::new();
        player_map.insert(host_session, host_id).unwrap();
        let mut broadcaster = LobbyBroadcaster::new();
        broadcaster.add_player_tx(host_id, host_tx);
        Self {
            lobby_code,
            player_map,
            broadcaster,
            lobby_tx,
            host: host_id,
            next_player_id: 1,
            settings: LobbySettings::default(),
            game_state: None,
        }
    }

    // could mean either player reconnected or joined
    pub fn session_connected(&mut self, session_id: SessionId, connection_tx: ConnectionTx) {
        // player already exists and reconnected (there is no way to reach this state by joining on two different devices simultaneously,
        // as they would've been hit with ErrorMessage::AlreadyInLobby at join attempt)
        if let Some(&player_id) = self.player_map.get_player_id(&session_id) {
            self.broadcaster.add_player_tx(player_id, connection_tx);
            self.broadcaster
                .broadcast(ServerMessage::PlayerBackOnline(player_id));
            self.broadcaster.send_to_player(
                &player_id,
                ServerMessage::ConnectedToLobby(self.info(player_id)),
            );
            return;
        }
        // player just joined
        let player_id = PlayerId::from(self.next_player_id);
        self.next_player_id += 1;
        self.player_map.insert(session_id, player_id).unwrap();
        self.broadcaster
            .add_player_tx(player_id, connection_tx.clone());
        self.broadcaster
            .broadcast(ServerMessage::PlayerJoined(player_id));
        self.broadcaster.send_to_player(
            &player_id,
            ServerMessage::ConnectedToLobby(self.info(player_id)),
        );
    }

    // returns true if the lobby should end, where it should have sent all close messages before that
    fn handle_lobby_message(&mut self, session_id: SessionId, message: LobbyMessage) -> bool {
        let player_id = *self.player_map.get_player_id(&session_id).unwrap();
        match message {
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
                if player_id != self.host {
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
                        if game_state.current_match.is_some() {
                            self.broadcaster.send_to_player(
                                &player_id,
                                ServerMessage::Error(ErrorMessage::AlreadyInMatch),
                            );
                            return false;
                        }
                        let Ok(current_match) = MatchState::new(
                            &self.player_map,
                            game_state.settings.n_cards_per_player,
                        ) else {
                            self.broadcaster.send_to_player(&player_id, ServerMessage::Error(ErrorMessage::Other("Failed to start round, settings.n_cards_per_player too high for card pool".to_string())));
                            return false;
                        };
                        self.broadcaster
                            .broadcast(ServerMessage::MatchStarted(current_match.info()));
                        game_state.current_match = Some(current_match);
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
                            None => match game_state.previous_matches.last() {
                                Some(match_state) => match_state.winner.unwrap(),
                                None => self.host,
                            },
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
                        if new_host != self.host && self.player_map.contains_player(&new_host) {
                            self.host = new_host;
                            self.broadcaster
                                .broadcast(ServerMessage::HostChanged(self.host));
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
                    HostMessage::CloseLobby => {
                        self.broadcaster
                            .broadcast(ServerMessage::LeftLobby(LeftLobbyReason::LobbyClosed));
                        return true;
                    }
                }
            }
        }
        false
    }

    fn handle_auto_message(&mut self, message: AutoMessage) {
        if let Some(ref mut game_state) = self.game_state {
            game_state.handle_auto_message(message);
        }
    }

    pub fn session_offline(&mut self, session_id: SessionId) {
        let player_id = *self.player_map.get_player_id(&session_id).unwrap();
        self.broadcaster.remove_player_tx(&player_id);
        self.broadcaster
            .broadcast(ServerMessage::PlayerOffline(player_id));
    }

    /// returns true if the lobby should end because all players left
    pub fn session_left(&mut self, session_id: SessionId, reason: LeftLobbyReason) -> bool {
        let player_id = self.player_map.remove_by_session_id(&session_id).unwrap();
        self.broadcaster.remove_player_tx(&player_id);

        self.broadcaster
            .broadcast(ServerMessage::PlayerLeft { player_id, reason });

        if player_id == self.host {
            let Some(new_host) = self.player_map.players().nth(0) else {
                // no players left in lobby, delete the lobby
                return true;
            };
            self.host = *new_host;
            self.broadcaster
                .broadcast(ServerMessage::HostChanged(self.host));
        }

        false
    }

    fn info(&self, player: PlayerId) -> LobbyInfo {
        LobbyInfo {
            you: player,
            lobby_code: self.lobby_code.clone(),
            players: self.player_map.players().copied().collect(),
            host: self.host,
            settings: self.settings.clone(),
            current_game: self.game_state.as_ref().map(|state| state.info()),
        }
    }
}

// this is fetched on connecting to a lobby
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyInfo {
    you: PlayerId,
    lobby_code: LobbyCode,
    players: Vec<PlayerId>,
    host: PlayerId,
    settings: LobbySettings,
    current_game: Option<GameInfo>,
}

pub enum LobbyMessage {
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
    CloseLobby,
    SetSettings(LobbySettings),
}

pub enum AutoMessage {
    ActionTimeout,
    AutoStartMatch,
    AutoStartRound,
}
pub enum InternalLobbyMessage {
    PlayerConnected {
        session_id: SessionId,
        connection_tx: ConnectionTx,
    },
    LobbyMessage {
        session_id: SessionId,
        message: LobbyMessage,
    },
    AutoMessage(AutoMessage),
    /// warns if the player's websocket session disconnected
    PlayerOffline(SessionId),
    PlayerLeft(SessionId),
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LeftLobbyReason {
    Left,
    LobbyClosed,
    KickedByHost,
    Disconnected,
}

/// the public incremental id everyone in the lobby knows
#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerId(usize);

impl From<usize> for PlayerId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Default)]
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
#[derive(Default)]
pub struct LobbyPlayers {
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
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.session_player.len()
    }
    pub fn insert(
        &mut self,
        session_id: SessionId,
        player_id: PlayerId,
    ) -> Result<(), LobbyPlayersInsertError> {
        match (
            self.session_player.contains_key(&session_id),
            self.player_session.contains_key(&player_id),
        ) {
            (true, true) => Err(LobbyPlayersInsertError::PlayerAndSessionAlreadyExists),
            (true, false) => Err(LobbyPlayersInsertError::SessionAlreadyExists),
            (false, true) => Err(LobbyPlayersInsertError::PlayerAlreadyExists),
            (false, false) => Ok(()),
        }?;
        self.session_player.insert(session_id, player_id);
        self.player_session.insert(player_id, session_id);
        Ok(())
    }
    // we dont need any smarter getters by index here, since this is just the lobby. in game
    // there will be the `Players` data structure instead
    pub fn players(&self) -> impl Iterator<Item = &PlayerId> {
        self.player_session.keys()
    }
    pub fn get_player_id(&self, session_id: &SessionId) -> Option<&PlayerId> {
        self.session_player.get(session_id)
    }
    pub fn session_tx_values(&self) -> impl Iterator<Item = &SessionId> {
        self.player_session.values()
    }
    pub fn remove_by_player_id(&mut self, player_id: &PlayerId) -> Option<SessionId> {
        let session_id = self.player_session.remove(player_id)?;
        self.session_player.remove(&session_id);
        Some(session_id)
    }
    pub fn remove_by_session_id(&mut self, session_id: &SessionId) -> Option<PlayerId> {
        let player_id = self.session_player.remove(session_id)?;
        self.player_session.remove(&player_id);
        Some(player_id)
    }

    pub fn contains_player(&self, player: &PlayerId) -> bool {
        self.player_session.contains_key(player)
    }
}
