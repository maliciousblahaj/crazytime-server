use crate::{
    ErrorMessage, ServerMessage, SessionId,
    game::{GameInfo, GameMessage, GameState, LobbySettings},
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
    host_id: SessionId,
    host_tx: ConnectionTx,
    mut lobby_rx: UnboundedReceiver<InternalLobbyMessage>,
    remove_session_tx: UnboundedSender<SessionId>,
) {
    let mut lobby = Lobby::new(host_id, host_tx);

    // set up a timer on disconnect message
    let disconnect_duration = Duration::from_secs(15);
    let mut disconnect_timer: Option<(SessionId, Pin<Box<Sleep>>)> = None;

    'lobby_runtime: loop {
        select! {
            _ = async { disconnect_timer.as_mut().unwrap().1.as_mut().await }, if disconnect_timer.is_some() => {
                let session_id = disconnect_timer.as_ref().unwrap().0;

                remove_session_tx.send(session_id);
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
                    InternalLobbyMessage::PlayerConnected { session_id, connection_tx } => {
                        lobby.session_connected(session_id, connection_tx);
                    },
                    InternalLobbyMessage::PlayerOffline(session_id) => {
                        disconnect_timer = Some((session_id, Box::pin(tokio::time::sleep(disconnect_duration))));
                        lobby.session_offline(session_id);
                    },
                    InternalLobbyMessage::PlayerLeft(session_id) => {

                        remove_session_tx.send(session_id);
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
    pub player_map: LobbyPlayers,
    pub host: PlayerId,

    /// next public player id, for incremental assignment
    pub next_player_id: usize,

    pub settings: LobbySettings,
    pub game_state: Option<GameState>,
}

impl Lobby {
    pub fn new(host_session: SessionId, host_tx: ConnectionTx) -> Self {
        let host_id = PlayerId::from(0);
        let mut player_map = LobbyPlayers::new();
        player_map.insert(host_session, host_id, host_tx);
        Self {
            player_map,
            host: host_id,
            next_player_id: 1,
            settings: LobbySettings::default(),
            game_state: None,
        }
    }

    // send a server message to all online players in the lobby
    pub fn broadcast(&self, message: ServerMessage) {
        for (_session, connection_tx) in self.player_map.session_tx_values() {
            if let Some(connection_tx) = connection_tx {
                connection_tx.send(message.clone());
            }
        }
    }

    // could mean either player reconnected or joined
    pub fn session_connected(&mut self, session_id: SessionId, connection_tx: ConnectionTx) {
        // player already exists and reconnected (there is no way to reach this state by joining on two different devices simultaneously,
        // as they would've been hit with ErrorMessage::AlreadyInLobby at join attempt)
        if let Some(&player_id) = self.player_map.get_player_id(&session_id) {
            self.player_map.add_tx(player_id, connection_tx);
            self.broadcast(ServerMessage::PlayerBackOnline(player_id));
            return;
        }
        // player just joined
        let player_id = PlayerId::from(self.next_player_id);
        self.next_player_id += 1;
        self.player_map
            .insert(session_id, player_id, connection_tx)
            .unwrap();
        self.broadcast(ServerMessage::PlayerJoined(player_id));
    }

    // returns true if the lobby should end, where it should have sent all close messages before that
    fn handle_lobby_message(&mut self, session_id: SessionId, message: LobbyMessage) -> bool {
        // this method will not be called without the player being joined in the lobby already
        let player_id = *self.player_map.get_player_id(&session_id).unwrap();
        // i wont take in the tx as a handle_message argument, since even if i could we still
        // store all tx's to be able to broadcast messages to all players, so we might as well
        // repurpose that store for other purposes.
        let connection_tx = self.player_map.get_tx(&player_id).unwrap().unwrap().clone();
        match message {
            LobbyMessage::GameMessage(game_message) => match self.game_state {
                Some(ref mut game_state) => {
                    game_state
                        .handle_message(player_id, game_message, connection_tx)
                        .inspect_err(|e| tracing::error!(error = %e));
                }
                None => {
                    connection_tx
                        .send(ServerMessage::Error(ErrorMessage::NotInGame))
                        .inspect_err(|e| tracing::error!(error = %e));
                }
            },
            LobbyMessage::HostMessage(host_message) => {
                if player_id != self.host {
                    connection_tx.send(ServerMessage::Error(ErrorMessage::InsufficientPermissions));
                    return false;
                }
                //
                match host_message {
                    HostMessage::StartGame => {
                        if self.game_state.is_some() {
                            connection_tx.send(ServerMessage::Error(ErrorMessage::AlreadyInGame));
                            return false;
                        }
                        self.game_state = Some(GameState::new()); // TODO: this constructor should take in all players, and more state as well
                        connection_tx.send(ServerMessage::GameStarted);
                        return false;
                    }
                    HostMessage::TransferHost(player_id) => {
                        if player_id != self.host {
                            self.host = player_id;
                            connection_tx.send(ServerMessage::HostChanged(self.host));
                            return false;
                        }
                    }
                    HostMessage::SetSettings(lobby_settings) => {
                        // checks if Acti should be updated,
                        // if so call self.game_state.set_active_game_settings
                        // as well
                        todo!()
                    }
                    // HostMessage::AddBot => {
                    //     // TODO, not implemented
                    // }
                    HostMessage::KickPlayer(player_id) => {
                        // check if player in game or match, if so call
                        // self.game_state.remove_player or the corresponding match variant
                        todo!()
                    }
                    HostMessage::CloseLobby => {
                        self.broadcast(ServerMessage::LeftLobby(LeftLobbyReason::LobbyClosed));
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn session_offline(&mut self, session_id: SessionId) {
        let player_id = *self.player_map.get_player_id(&session_id).unwrap();
        self.player_map.remove_tx(player_id);
        self.broadcast(ServerMessage::PlayerOffline(player_id));
    }

    /// returns true if the lobby should end because all players left
    pub fn session_left(&mut self, session_id: SessionId, reason: LeftLobbyReason) -> bool {
        let player_id = self.player_map.remove_by_session_id(&session_id).unwrap();
        self.broadcast(ServerMessage::PlayerLeft { player_id, reason });

        if player_id == self.host {
            let Some(new_host) = self.player_map.players().nth(0) else {
                // no players left in lobby, delete the lobby
                return true;
            };
            self.host = *new_host;
            self.broadcast(ServerMessage::HostChanged(self.host));
        }

        false
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

// these message types are public and should directly correspond with client messages
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LobbyMessage {
    GameMessage(GameMessage),
    HostMessage(HostMessage),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostMessage {
    StartGame,
    TransferHost(PlayerId),
    // not implemented
    // AddBot,
    KickPlayer(PlayerId),
    CloseLobby,
    SetSettings(LobbySettings),
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

// the reason for why we store tx for each player in here is to support broadcasting to all players
//
// if the player is offline, the tx will be None

#[derive(Default)]
pub struct LobbyPlayers {
    session_player: HashMap<SessionId, PlayerId>,
    player_session_tx: BTreeMap<PlayerId, (SessionId, Option<ConnectionTx>)>,
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
    pub fn insert(
        &mut self,
        session_id: SessionId,
        player_id: PlayerId,
        tx: ConnectionTx,
    ) -> Result<(), LobbyPlayersInsertError> {
        match (
            self.session_player.contains_key(&session_id),
            self.player_session_tx.contains_key(&player_id),
        ) {
            (true, true) => Err(LobbyPlayersInsertError::PlayerAndSessionAlreadyExists),
            (true, false) => Err(LobbyPlayersInsertError::SessionAlreadyExists),
            (false, true) => Err(LobbyPlayersInsertError::PlayerAlreadyExists),
            (false, false) => Ok(()),
        }?;
        self.session_player.insert(session_id, player_id);
        self.player_session_tx
            .insert(player_id, (session_id, Some(tx)));
        Ok(())
    }
    // we dont need any smarter getters by index here, since this is just the lobby. in game
    // there will be the `Players` data structure instead
    pub fn players(&self) -> impl Iterator<Item = &PlayerId> {
        self.player_session_tx.keys()
    }
    pub fn get_player_id(&self, session_id: &SessionId) -> Option<&PlayerId> {
        self.session_player.get(session_id)
    }
    pub fn session_tx_values(&self) -> impl Iterator<Item = &(SessionId, Option<ConnectionTx>)> {
        self.player_session_tx.values()
    }
    pub fn get_tx(&self, player_id: &PlayerId) -> Option<Option<&ConnectionTx>> {
        self.player_session_tx
            .get(player_id)
            .map(|s_t| s_t.1.as_ref())
    }
    /// if a player disconnects, remove the dead tx
    pub fn remove_tx(&mut self, player_id: PlayerId) {
        self.player_session_tx.entry(player_id).and_modify(|s_t| {
            s_t.1 = None;
        });
    }
    /// if a player reconnects, add back their tx
    pub fn add_tx(&mut self, player_id: PlayerId, connection_tx: ConnectionTx) {
        self.player_session_tx.entry(player_id).and_modify(|s_t| {
            s_t.1 = Some(connection_tx);
        });
    }
    pub fn remove_by_player_id(
        &mut self,
        player_id: &PlayerId,
    ) -> Option<(SessionId, Option<ConnectionTx>)> {
        let (session_id, tx) = self.player_session_tx.remove(player_id)?;
        self.session_player.remove(&session_id);
        Some((session_id, tx))
    }
    pub fn remove_by_session_id(&mut self, session_id: &SessionId) -> Option<PlayerId> {
        let player_id = self.session_player.remove(session_id)?;
        self.player_session_tx.remove(&player_id);
        Some(player_id)
    }
}
