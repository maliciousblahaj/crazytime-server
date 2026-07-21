use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{
    game::{
        ActiveGameSettings, LobbySettings,
        r#match::MatchInfo,
        round::{
            InputPlayerAction, PlayerAction, PlayerActionType, PlayerLostReason,
            RoundTerminationType,
        },
    },
    lobby::{LeftLobbyReason, LobbyCode, LobbyInfo, LobbyMessage, PlayerId},
    rules::Description,
};

pub mod card;
pub mod game;
pub mod lobby;
pub mod rules;
pub mod session;

/// a message sent from the server to a client
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerMessage {
    // only sent on server termination (like ctrl+C)
    ServerClosed,

    // lobby

    // this is provided whenever a users connects to a lobby
    ConnectedToLobby(LobbyInfo),

    // LeftLobbyReason is not perfect, as the Disconnected variant will never be constructed (since if you're disconnected you
    // wont be able to receive that message anyways), but making two separate enums for representing this mostly overlapping
    // kind of state is more than it is worth in my opinion, at least right now.
    LeftLobby(LeftLobbyReason),

    // this updates the lobby settings, and will still apply to the current game if it also causes ActiveGameSettings to update
    LobbySettingsUpdated(LobbySettings),
    PlayerJoined(PlayerId),
    PlayerOffline(PlayerId),
    PlayerBackOnline(PlayerId),
    PlayerLeft {
        player_id: PlayerId,
        reason: LeftLobbyReason,
    },
    HostChanged(PlayerId),

    // game
    GameStarted, // will most likely also send MatchStarted at the same time, though not RoundStarted obviously
    GameEnded,
    // if a lobby setting change triggers the current game
    ActiveGameSettingsUpdated(ActiveGameSettings),
    RuleAdded {
        criteria: Description,
        rule: Description,
    },

    // match
    MatchStarted(MatchInfo),
    MatchEnded,
    PlayerGotRidOfCardsToPool {
        player_id: PlayerId,
        n_cards: usize,
    },
    PlayerPickedUpRevealedCards {
        player_id: PlayerId,
        // i choose to not make this a Vec<Card>, even though it technically
        // is publically feasible to figure which cards are picked up out in
        // many situations. but i will make it harder by randomizing which ones
        // of the revealed cards were actually picked up, like the player never
        // saw the picking up motion, just that all revealed cards are now gone
        // and both the user's pile has grown by e.g 5 cards, and the pool by 3.
        //
        // only problem i see with this being an usize instead of Vec<Card> is that
        // maybe the usize might be bigger than the actual amount of cards revealed,
        // but ill just make sure to never induce this state in code.
        n_cards: usize,
    },

    // round
    RoundStarted {
        starting_player: PlayerId,
    },
    RoundEnded(RoundTerminationType),
    ActionPerformed(PlayerAction),

    // response
    Error(ErrorMessage),
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorMessage {
    NotInLobby,
    NotInGame,
    NotInMatch,
    NotInRound,
    // if you try to lay a card but you don't have any
    NoCards,
    AlreadyInLobby,
    LobbyDoesNotExist,
    // if you are not host and try send host messages
    InsufficientPermissions,
    AlreadyInGame,
}

/// a message sent from a client to the server
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClientMessage {
    JoinLobby(LobbyCode),
    HostLobby,
    LeaveLobby,

    // lobby
    StartGame,
    TransferHost(PlayerId),
    // not implemented
    // AddBot,
    KickPlayer(PlayerId),
    CloseLobby,
    SetSettings(LobbySettings),
    // game
    AddNewRule,
    RemoveRule(usize),

    // round
    PerformAction(InputPlayerAction),
}

/// corresponds exactly to the session id of the user, used to authenticate
#[derive(Clone, Copy, Eq, PartialEq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionId(u128);

impl SessionId {
    pub fn new() -> Self {
        Self(rand::random())
    }
}
