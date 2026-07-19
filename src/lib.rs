use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{
    game::{
        ActiveGameSettings, GameSettings,
        r#match::InitMatchState,
        round::{InitRoundState, PlayerActionType, PlayerLostReason},
    },
    lobby::{LeftLobbyReason, LobbyCode, LobbyInfo, PlayerId},
    rules::Description,
};

pub mod card;
pub mod game;
pub mod lobby;
pub mod rules;
pub mod session;

#[derive(Serialize)]
pub enum ServerMessage {
    // session
    JoinedLobby(LobbyCode),

    // lobby

    // this is provided whenever a users connection is established and they are already in a lobby (like if they disconnected),
    // as well as when they join a lobby,
    LobbyInfo(LobbyInfo),

    LobbyClosed,
    // LeftLobbyReason is not perfect, as the Disconnected variant will never be constructed (since if you're disconnected you
    // wont be able to receive that message anyways), but making two separate enums for representing this mostly overlapping
    // kind of state is more than it is worth in my opinion, at least right now.
    LeftLobby(LeftLobbyReason),

    // this can also be updated while in the lobby, although some are not guaranteed
    // to have effect on the current game, if it does change something, ActiveGameSettingsUpdated
    // will fire
    GameSettingsUpdated(GameSettings),
    PlayerJoined(PlayerId),
    PlayerLeft {
        player: PlayerId,
        reason: LeftLobbyReason,
    },
    HostChanged(PlayerId),

    // game
    GameStarted,
    GameEnded,
    // if a game setting change triggers the current game
    ActiveGameSettingsUpdated(ActiveGameSettings),
    RuleAdded {
        criteria: Description,
        rule: Description,
    },

    // match
    MatchStarted(InitMatchState),
    MatchEnded,
    PlayerPickedUpCards {
        player: PlayerId,
        // i choose to not make this a Vec<Card>, even though it technically
        // is publically feasible to figure which cards are picked up out in
        // many situations. but i will make it harder by randomizing which ones
        // of the revealed cards were actually picked up, like the player never
        // saw the picking up motion, just that all revealed cards are now gone
        // and both the user's pile has grown by e.g 5 cards, and the pool by 3.
        n_cards: usize,
    },
    PlayerGotRidOfCardsToPool {
        player: PlayerId,
        n_cards: usize,
    },

    // round
    RoundStarted(InitRoundState),
    RoundEnded {
        loser: PlayerId,
        reason: PlayerLostReason,
    },
    ActionPerformed {
        player: PlayerId,
        action: PlayerActionType,
    },

    // response
    Error(ErrorMessage),
}

#[derive(Serialize, Deserialize)]
pub enum ErrorMessage {
    NotInLobby,
    NotInRound,
    NotInGame,
    AlreadyInLobby,
    LobbyDoesNotExist,
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

impl Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

pub struct SessionInfo {
    current_lobby: Option<LobbyCode>,
}
