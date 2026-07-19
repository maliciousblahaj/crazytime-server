use crate::{
    ErrorMessage, ServerMessage, SessionId,
    game::{GameInfo, GameMessage, GameSettings, GameState},
};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub async fn lobby_task(
    host_id: SessionId,
    host_tx: ConnectionTx,
    mut lobby_rx: UnboundedReceiver<InternalLobbyMessage>,
    remove_session_tx: UnboundedSender<SessionId>,
) {
    let mut lobby = Lobby::new(host_id, host_tx);

    // set up a timer on disconnect message

    while let Some(message) = lobby_rx.recv().await {
        // returns true if the lobby is finished
        if lobby.handle_message(message).await {
            break;
        }
    }
}

pub struct Lobby {
    pub player_map: LobbyPlayers,
    pub host: PlayerId,

    /// next public player id, for incremental assignment
    pub next_player_id: usize,

    pub game_settings: GameSettings,
    pub game_state: Option<GameState>,
}

// maybe i should not keep a consistent host_tx. because if the thread can into the channel send a session id
// each time as well, it might as well also send a ConnectionTx each time, and we wont have to deal with cleaning
// up things. not even on potential disconnect we need to store the tx, as there will be noone to send the kicked
// message to anyways
impl Lobby {
    pub fn new(host_session: SessionId, host_tx: ConnectionTx) -> Self {
        let host_id = PlayerId::from(0);
        let mut player_map = LobbyPlayers::new();
        player_map.insert(host_session, host_id, host_tx);
        Self {
            player_map,
            host: host_id,
            next_player_id: 1,
            game_settings: GameSettings::default(),
            game_state: None,
        }
    }

    /// returns true if the lobby should end, but it should have sent all close messages before that
    pub async fn handle_message(&mut self, message: InternalLobbyMessage) -> bool {
        match message {
            InternalLobbyMessage::LobbyMessage {
                session_id,
                message,
            } => todo!(),
            InternalLobbyMessage::AddPlayer(session_id) => {}
            InternalLobbyMessage::RemovePlayer(session_id) => {}
            InternalLobbyMessage::PlayerDisconnected(session_id) => todo!(),
        }
        // this method will not be called without the player being joined in the lobby already
        let player_id = self.player_map.get_player_id(&session_id).unwrap();
        // i wont take in the tx as a handle_message argument, since even if i could we still
        // store all tx's to be able to broadcast messages to all players, so we might as well
        // repurpose that store for other purposes.
        let tx = self.player_map.get_tx(player_id).unwrap();
        match message {
            LobbyMessage::GameMessage(game_message) => match self.game_state {
                Some(ref mut game_state) => {
                    game_state
                        .handle_message(player_id, game_message, tx)
                        .await
                        .inspect_err(|e| tracing::error!(error = %e));
                }
                None => {
                    tx.send(ServerMessage::Error(ErrorMessage::NotInGame))
                        .inspect_err(|e| tracing::error!(error = %e));
                }
            },
            LobbyMessage::StartGame => todo!(),
            LobbyMessage::TransferHost(player_id) => todo!(),
            LobbyMessage::SetGameSettings(game_settings) => {
                // checks if ActiveGameSettings should be updated,
                // if so call self.game_state.set_active_game_settings
                // as well
                todo!()
            }
            LobbyMessage::AddBot => {
                // TODO
            }
            LobbyMessage::KickPlayer(player_id) => {
                // check if player in game or match, if so call
                // self.game_state.remove_player or the corresponding match variant
            }
            LobbyMessage::EndLobby => {}
            LobbyMessage::AddPlayer(player_id) => todo!(),
            LobbyMessage::RemovePlayer(player_id) => todo!(),
        }
        Ok(())
    }
}

pub type ConnectionTx = UnboundedSender<ServerMessage>;

// this is fetched on joining a lobby, or reconnecting to a lobby
#[derive(Serialize)]
pub struct LobbyInfo {
    you: PlayerId,
    lobby_code: LobbyCode,
    players: Vec<PlayerId>,
    host: PlayerId,
    game_settings: GameSettings,
    current_game: Option<GameInfo>,
}

pub enum AddPlayerToLobbyError {
    LobbyPlayersInsertError(LobbyPlayersInsertError),
}

impl Lobby {
    // anytime a player joins a lobby and connects to it
    pub fn add_player(
        &mut self,
        session_id: SessionId,
        tx: ConnectionTx,
    ) -> Result<(), AddPlayerToLobbyError> {
        let player_id = PlayerId::from(self.next_player_id);
        self.next_player_id += 1;
        self.player_map
            .insert(session_id, player_id, tx)
            .map_err(AddPlayerToLobbyError::LobbyPlayersInsertError)?;
        Ok(())
    }
}

// these message types are public and should directly correspond with client messages,
// to have private behavior, call ordinary functions instead, like
// self.game_state.unwrap().add_new_rule() or whatever. if this is identical to message
// handling behavior, still put it in a function and call it from the message handler
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LobbyMessage {
    StartGame,
    TransferHost(PlayerId),
    SetGameSettings(GameSettings),
    AddBot,
    KickPlayer(PlayerId),
    CloseLobby,

    GameMessage(GameMessage),
}

pub enum InternalLobbyMessage {
    LobbyMessage {
        session_id: SessionId,
        message: LobbyMessage,
    },
    PlayerConnected {
        session_id: SessionId,
        connection_tx: ConnectionTx,
    },
    PlayerDisconnected(SessionId),
    PlayerLeft(SessionId),
}

#[derive(Serialize)]
pub enum LeftLobbyReason {
    Left,
    KickedByHost,
    Disconnected,
}

/// the public id everyone in the lobby knows
#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

// the reason for why we store tx for each player in here is that obviously most lobby events that
// are initiated by one player will be broadcasted to many other players.

#[derive(Default)]
pub struct LobbyPlayers {
    session_player: HashMap<SessionId, PlayerId>,
    player_session_tx: BTreeMap<PlayerId, (SessionId, ConnectionTx)>,
}
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
        self.player_session_tx.insert(player_id, (session_id, tx));
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
    // pub fn get_session_id_tx(
    //     &self,
    //     player_id: &PlayerId,
    // ) -> Option<&(SessionId, ConnectionTx)> {
    //     self.player_session_tx.get(player_id)
    // }
    pub fn get_tx(&self, player_id: &PlayerId) -> Option<&ConnectionTx> {
        self.player_session_tx.get(player_id).map(|s_t| &s_t.1)
    }
    pub fn remove_by_player_id(
        &mut self,
        player_id: &PlayerId,
    ) -> Option<(SessionId, ConnectionTx)> {
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

// im just writing this here to reason about the representation.
//
// so every single player knows everything as every other player. all information
// someone knows is public, and will be continuously updated from every server message.
// If a user performs an action the server will respond by broadcasting their action
// to everyone, including themselves, and only then should the ui update.
//
// what do the criterias know? the criterias know literally exactly all information that
// is public, and has been accumulated from all actions. this means the server must keep
// all state that is accumulated to the same degree as the frontend will do, else the
// frontend might know more than the server does. And the server will update its internal
// "frontend" of public information when sending every action, just like the frontend
// when it receives every action, and that internal state update could literally be a
// function which takes in a borrowed servermessage and processes it to update internal
// state, like a mock frontend.
//
// in this same mock frontend is also where the expected things are stored, since that *is*
// public knowledge, just not explicit, and its literally the entire gameplay to keep track
// of this. here we also store if an error happened. we only need public information to know
// this.
//
// in fact we never need private information for anything, the only private thing is each
// players' card hands, which noone needs to know, and can almost be abstracted away as a
// random pool.
