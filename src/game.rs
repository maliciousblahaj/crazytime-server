use std::sync::mpsc::SendError;

use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    ServerMessage,
    game::r#match::{FinishedMatch, MatchInfo, MatchMessage, MatchState},
    lobby::PlayerId,
    rules::{ActiveRuleInfo, RuleManager},
};

pub mod r#match;
pub mod round;

#[derive(Serialize)]
pub struct GameSettings {}

impl Default for GameSettings {
    fn default() -> Self {
        Self {}
    }
}
#[derive(Serialize)]
pub struct ActiveGameSettings {}

pub struct GameState {
    pub settings: ActiveGameSettings,

    // in fact we dont even need a player set here, since there wont be any case where
    // a player is in a lobby but not in a game, so we'll simply have it equal the
    // lobby set
    // // here we can just use a BTreeSet since we don't need random gets in O(1),
    // // while in Match we will need O(1) index->player and O(logn) player->index
    // pub players: BTreeSet<PlayerId>,
    /// all previous matches saved
    pub current_match: Option<MatchState>,
    pub previous_matches: Vec<FinishedMatch>,
    pub rule_manager: RuleManager,
}
impl GameState {
    pub fn new() -> Self {
        todo!()
    }
    pub fn handle_message(
        &mut self,
        player_id: PlayerId,
        message: GameMessage,
        tx: UnboundedSender<ServerMessage>,
    ) -> Result<(), SendError<ServerMessage>> {
        Ok(())
    }
}

// fetched on request
#[derive(Serialize)]
pub struct GameInfo {
    settings: ActiveGameSettings,
    active_rules: Vec<ActiveRuleInfo>,
    current_match: Option<MatchInfo>,
    last_match_winner: Option<PlayerId>,
}

pub enum GameMessage {
    MatchMessage(MatchMessage),
}

impl GameState {
    pub fn process_message(message: GameMessage) {
        todo!()
    }
}
