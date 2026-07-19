use std::{collections::HashMap, sync::mpsc::SendError, time::Duration};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    ServerMessage,
    card::Card,
    game::{
        r#match::{
            FinishedMatch, InitMatchState, MatchInfo, MatchMessage, MatchPlayers, MatchState,
        },
        round::{FinishedRound, InitRoundState, PlayerAction, PlayerLostReason},
    },
    lobby::PlayerId,
    rules::{RuleInfo, RuleManager},
};

pub mod r#match;
pub mod round;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbySettings {
    /// the max number of players for a lobby
    pub max_players: usize,

    /// the time a player is allowed to execute a correct action, if not another player had intervened with
    /// and error. For example if you make a correct move in your turn, but almost just slightly before
    /// you, some other player moved out of their turn or hit in the middle, and since you made your move
    /// after their error you will have made an even bigger error by moving after their error and not reporting
    /// it. same if it was a hit pile and they made the wrong hit type but you obviously didnt notice in time.
    /// This is not intended behavior, and therefore this duration exists for the judging algorithm to determine
    /// if you made an error in your "correct" move or not, based on how much time passed since the error occured.
    /// If your move would've been incorrect even if that other error didn't happen, this doesn't apply and you'll
    /// still be judged to have made the biggest error.
    pub expected_error_reaction_time: Duration,

    /// if you report an error correctly, how many cards will you get rid of from your pile
    pub cards_removed_at_correct_error_report: usize,

    /// how many of the revealed cards you will pick up if you lose the round
    pub max_cards_picked_up_when_losing: MaxCardsPickedUpWhenLosing,

    /// how long an action should take before the player loses on timeout
    pub action_timeout_rate: Duration,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MaxCardsPickedUpWhenLosing {
    Finite(usize),
    Unlimited,
}

impl Default for LobbySettings {
    fn default() -> Self {
        Self {
            max_players: 10,
            expected_error_reaction_time: Duration::from_secs(2),
            cards_removed_at_correct_error_report: 0,
            max_cards_picked_up_when_losing: MaxCardsPickedUpWhenLosing::Finite(5),
            action_timeout_rate: Duration::from_secs(10),
        }
    }
}
/// game settings within a game
#[derive(Clone, Serialize)]
pub struct ActiveGameSettings {
    /// See [`GameSettings.expected_error_reaction_time`]
    pub expected_error_reaction_time: Duration,
    /// See [`GameSettings.cards_removed_at_correct_error_report`]
    pub cards_removed_at_correct_error_report: usize,
    /// See [`GameSettings.max_cards_picked_up_when_losing`]
    pub max_cards_picked_up_when_losing: MaxCardsPickedUpWhenLosing,
    /// See [`GameSettings.action_timeout_rate`]
    pub action_timeout_rate: Duration,
}

impl ActiveGameSettings {
    pub fn update(&mut self, new_settings: LobbySettings) {
        self.expected_error_reaction_time = new_settings.expected_error_reaction_time;
        self.cards_removed_at_correct_error_report =
            new_settings.cards_removed_at_correct_error_report;
    }
}

impl From<LobbySettings> for ActiveGameSettings {
    fn from(value: LobbySettings) -> Self {
        Self {
            expected_error_reaction_time: value.expected_error_reaction_time,
            cards_removed_at_correct_error_report: value.cards_removed_at_correct_error_report,
            max_cards_picked_up_when_losing: value.max_cards_picked_up_when_losing,
            action_timeout_rate: value.action_timeout_rate,
        }
    }
}

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
    pub fn lobby_settings_updated(
        &mut self,
        lobby_settings: LobbySettings,
    ) -> Option<ActiveGameSettings> {
        todo!()
    }
    // /// can only be run during a round, else will return None
    // pub fn rule_input(&mut self) -> Option<(GameContext, MutableRoundState)> {
    //     let Some(match_state) = self.current_match else {
    //         return None;
    //     };
    //     let Some(round_state) = match_state.current_round else {
    //         return None;
    //     };

    //     Some((GameContext {
    //         players: &match_state.players,
    //         n_cards_in_pool: match_state.card_pool.n_cards(),
    //         previous_matches: &self.previous_matches,
    //         previous_rounds: &match_state.previous_rounds,
    //         init_match_state: &match_state.init_match_state,
    //         init_round_state: &round_state.init_round_state,
    //         player_actions: &round_state.player_actions,
    //         public_card_stacks: &round_state.public_card_stacks,
    //         error_occured: &round_state.error_occured,
    //     }, MutableRoundState {

    //         })
    // }
}
pub struct GameContext<'a> {
    pub players: &'a MatchPlayers,
    pub n_cards_in_pool: usize,
    pub previous_matches: &'a Vec<FinishedMatch>,
    pub previous_rounds: &'a Vec<FinishedRound>,
    pub init_match_state: &'a InitMatchState,
    pub init_round_state: InitRoundState,
    /// every move/hit that is made is pushed onto this stack
    pub player_actions: &'a Vec<PlayerAction>,
    /// the cards each player has revealed this round
    pub public_card_stacks: &'a HashMap<PlayerId, Vec<Card>>,
    /// the index of player_actions when it occured, and the reason
    pub error_occured: Option<(usize, PlayerLostReason)>,
}

// fetched on request
#[derive(Clone, Serialize)]
pub struct GameInfo {
    settings: ActiveGameSettings,
    active_rules: Vec<RuleInfo>,
    current_match: Option<MatchInfo>,
    last_match_winner: Option<PlayerId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameMessage {
    MatchMessage(MatchMessage),
}

impl GameState {
    pub fn process_message(message: GameMessage) {
        todo!()
    }
}
