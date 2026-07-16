use crate::{
    ServerMessage,
    card::{Card, CardPool},
    game::{
        PlayerId,
        round::{FinishedRound, RoundInfo, RoundMessage, RoundState},
    },
};
use serde::Serialize;
use sorted_vec::SortedVec;
use std::{collections::HashMap, sync::mpsc::SendError};
use tokio::sync::mpsc::UnboundedSender;

pub struct MatchState {
    pub init_match_state: InitMatchState,
    pub card_pool: CardPool,
    pub players: MatchPlayers,
    pub finished_rounds: Vec<FinishedRound>,
    pub current_round: Option<RoundState>,
}

pub enum MatchMessage {
    RoundMessage(RoundMessage),
}

impl MatchState {
    pub async fn handle_message(
        player: PlayerId,
        message: MatchMessage,
        tx: UnboundedSender<ServerMessage>,
    ) -> Result<(), SendError<ServerMessage>> {
        Ok(())
    }
}

/// is sent in LobbyInfo on request
#[derive(Serialize)]
pub struct MatchInfo {
    players: Vec<(PlayerId, usize)>,
    n_cards_in_pool: usize,
    current_round: Option<RoundInfo>,
}

/// is sent when a new match starts
#[derive(Serialize)]
pub struct InitMatchState {
    // how many cards in their private pile
    players: Vec<(PlayerId, usize)>,
    n_cards_in_pool: usize,
}

/// A set of all players in a match, including their card hands,
/// with O(1) index->player lookup, O(logN) player->index lookup,
/// O(N) insertion, and uniqueness constraint
#[derive(Default)]
pub struct MatchPlayers {
    index: SortedVec<PlayerId>,
    hands: HashMap<PlayerId, Vec<Card>>,
}

impl MatchPlayers {
    pub fn new() -> Self {
        Self::default()
    }
    /// returns (false, index) if the player already exists at a certain index,
    /// else (true, index) if the player was just inserted at a certain index.
    pub fn add_player(&mut self, player: PlayerId, hand: Vec<Card>) -> (bool, usize) {
        match self.index.find_or_insert(player) {
            sorted_vec::FindOrInsert::Found(index) => (false, index),
            sorted_vec::FindOrInsert::Inserted(index) => {
                self.hands.insert(player, hand);
                (true, index)
            }
        }
    }
    /// returns their card hands if the player was removed, and None if they
    /// don't exist in the set
    pub fn remove_player(&mut self, player: &PlayerId) -> Option<Vec<Card>> {
        self.index
            .remove_item(player)
            .map(|player| self.hands.remove(&player).unwrap())
    }
}

// internal structs
pub struct FinishedMatch {
    /// ordered placing, from first to last place. item 0 is the winner of the round
    /// contains all players, and therefore a players field is redundant
    pub placings: Vec<PlayerId>,
    ///
    pub card_pool: CardPool,
    pub rounds: Vec<FinishedRound>,
}
