use std::collections::{BTreeMap, HashMap};

use rand::RngExt;
use serde::{Deserialize, Serialize};

use crate::{
    game::GameState,
    player::{PlayerId, PlayerState},
};

#[derive(Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
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

pub struct Lobby {
    lobby_code: LobbyCode,

    /// next public player id, for incremental assignment
    next_player_id: usize,
    /// map public player id to private internally
    player_map: BTreeMap<usize, PlayerId>,
    players: HashMap<PlayerId, PlayerState>,

    host: PlayerId,
    game_state: GameState,
}

impl Lobby {
    pub fn new(lobby_code: LobbyCode, host: PlayerId) -> Self {
        let host_public_id = 0;
        Self {
            lobby_code,
            next_player_id: 1,
            player_map: BTreeMap::from([(host_public_id, host)]),
            players: HashMap::from([(host, PlayerState::new(host, host_public_id))]),
            host,
            game_state: GameState::new(),
        }
    }

    // players can be added between rounds, but each round has a list of players,
    // which doesn't change, only when a new round is started will it change to
    // include the player
    //
    // anytime a player joins a lobby and connects to it
    pub fn add_player(player_id: PlayerId) -> Result<(), ()> {
        todo!()
    }
}
