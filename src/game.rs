use std::collections::HashMap;

use chrono::{Duration, Utc};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::{
    ErrorMessage, ServerMessage,
    card::Card,
    game::{
        r#match::{MatchInfo, MatchPlayers, MatchState},
        round::{
            InputPlayerAction, InputPlayerMove, PlayerAction, PlayerActionType, PlayerMove,
            RoundState, RoundTerminationType,
        },
    },
    lobby::{LobbyBroadcaster, PlayerId},
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
            expected_error_reaction_time: Duration::seconds(2),
            cards_removed_at_correct_error_report: 0,
            max_cards_picked_up_when_losing: MaxCardsPickedUpWhenLosing::Finite(5),
            action_timeout_rate: Duration::seconds(10),
        }
    }
}
/// game settings within a game
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveGameSettings {
    /// See [`LobbySettings.expected_error_reaction_time`]
    pub expected_error_reaction_time: Duration,
    /// See [`LobbySettings.cards_removed_at_correct_error_report`]
    pub cards_removed_at_correct_error_report: usize,
    /// See [`LobbySettings.max_cards_picked_up_when_losing`]
    pub max_cards_picked_up_when_losing: MaxCardsPickedUpWhenLosing,
    /// See [`LobbySettings.action_timeout_rate`]
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
    pub previous_matches: Vec<MatchState>,
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
        broadcaster: &LobbyBroadcaster,
    ) {
        match message {
            GameMessage::ActionPerformed(action) => {
                let Some(ref mut current_match) = self.current_match else {
                    broadcaster
                        .send_to_player(&player_id, ServerMessage::Error(ErrorMessage::NotInMatch));
                    return;
                };
                let Some(ref mut current_round) = current_match.current_round else {
                    broadcaster
                        .send_to_player(&player_id, ServerMessage::Error(ErrorMessage::NotInRound));
                    return;
                };
                // parse the action and broadcast it
                let time = Utc::now();
                let player_action_type = match action {
                    InputPlayerAction::Move(input_player_move) => {
                        let player_move = match input_player_move {
                            InputPlayerMove::CountAndLayCard(count) => {
                                let Some(card) = current_match.players.take_card(&player_id) else {
                                    broadcaster.send_to_player(
                                        &player_id,
                                        ServerMessage::Error(ErrorMessage::NoCards),
                                    );
                                    return;
                                };
                                PlayerMove::CountAndLayCard { card, count }
                            }
                            InputPlayerMove::Count(count) => PlayerMove::Count(count),
                            InputPlayerMove::LayCard => {
                                let Some(card) = current_match.players.take_card(&player_id) else {
                                    broadcaster.send_to_player(
                                        &player_id,
                                        ServerMessage::Error(ErrorMessage::NoCards),
                                    );
                                    return;
                                };
                                PlayerMove::LayCard(card)
                            }
                        };
                        PlayerActionType::Move(player_move)
                    }
                    InputPlayerAction::Hit(hit_type) => PlayerActionType::Hit(hit_type),
                    InputPlayerAction::ReportError => PlayerActionType::ReportError,
                    InputPlayerAction::DeclareWin => PlayerActionType::DeclareWin,
                };
                let player_action = PlayerAction {
                    player_id,
                    time,
                    r#type: player_action_type,
                };
                current_round.player_actions.push(player_action.clone());
                broadcaster.broadcast(ServerMessage::ActionPerformed(player_action));

                // check rules and do server verification of action, and broadcast accordingly
                if let Some(round_termination) = current_round.action_chain.process_action(
                    player_id,
                    action,
                    &self.settings,
                    &current_match.players,
                    time,
                ) {
                    broadcaster.broadcast(ServerMessage::RoundEnded(round_termination.clone()));
                    if let Some(guilty) = match round_termination {
                        RoundTerminationType::ErrorReported { reporter, errors } => {
                            if self.settings.cards_removed_at_correct_error_report > 0 {
                                let cards = current_match.players.take_cards(
                                    &reporter,
                                    self.settings.cards_removed_at_correct_error_report,
                                );
                                if !cards.is_empty() {
                                    let n_cards = cards.len();
                                    current_match.card_pool.add_cards(cards.into_iter());
                                    broadcaster.broadcast(
                                        ServerMessage::PlayerGotRidOfCardsToPool {
                                            player_id: reporter,
                                            n_cards,
                                        },
                                    );
                                }
                            }
                            Some(errors.last().unwrap().player)
                        }
                        RoundTerminationType::FaultyErrorReport(player_id) => Some(player_id),
                        RoundTerminationType::HitPileLast(player_id) => Some(player_id),
                        RoundTerminationType::FaultyWinDeclaration(player_id) => Some(player_id),
                        RoundTerminationType::PlayerWonMatch(player_id) => {
                            // make the match end
                            None
                        }
                    } {
                        let mut revealed_cards: Vec<Card> = current_round
                            .public_card_stacks
                            .iter()
                            .fold(Vec::new(), |mut acc, card_stack| {
                                acc.extend(card_stack.1);
                                acc
                            });
                        revealed_cards.shuffle(&mut rand::rng());
                        let picked_up_cards = match self.settings.max_cards_picked_up_when_losing {
                            MaxCardsPickedUpWhenLosing::Finite(n) => {
                                revealed_cards.split_off(revealed_cards.len().saturating_sub(n))
                            }
                            MaxCardsPickedUpWhenLosing::Unlimited => revealed_cards,
                        };
                    }
                    // round is finished
                    // broadcast finished round and loser
                    // if playerwon end match as well
                    // add to losers card pile from pool
                    // get rid of 1 card off error reporter
                    return;
                }
            }
            GameMessage::MoveTimeout => todo!(),
            GameMessage::AddNewRule => todo!(),
            GameMessage::RemoveRule(_) => todo!(),
        }
    }
    pub fn lobby_settings_updated(
        &mut self,
        lobby_settings: LobbySettings,
    ) -> Option<ActiveGameSettings> {
        todo!()
    }
}

// criterias use this to determine whether to launch a RuleEffect,
// and RuleEffects also use this when running
pub struct GameContext<'a> {
    pub players: &'a MatchPlayers,
    pub n_cards_in_pool: usize,
    pub previous_matches: &'a Vec<MatchState>,
    pub previous_rounds: &'a Vec<RoundState>,
    pub round_starting_player: &'a PlayerId,
    /// every move/hit that is made is pushed onto this stack
    pub player_actions: &'a Vec<PlayerAction>,
    /// the cards each player has revealed this round
    pub public_card_stacks: &'a HashMap<PlayerId, Vec<Card>>,
}

// fetched on request
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    settings: ActiveGameSettings,
    active_rules: Vec<RuleInfo>,
    current_match: Option<MatchInfo>,
    last_match_winner: Option<PlayerId>,
}

//these are internal messages
pub enum GameMessage {
    ActionPerformed(InputPlayerAction),
    MoveTimeout,
    AddNewRule,
    RemoveRule(usize),
}
