use std::collections::HashMap;

use chrono::{Duration, Utc};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    ErrorMessage, ServerMessage,
    card::Card,
    game::{
        r#match::{MatchInfo, MatchState},
        round::{
            Count, InputPlayerAction, InputPlayerMove, PlayerAction, PlayerActionType, PlayerMove,
            RoundState, RoundTerminationType,
        },
    },
    lobby::{AutoMessage, InternalLobbyMessage, LobbyBroadcaster, PlayerId},
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

    /// when a match starts, how many cards are handed out to each player
    pub n_cards_per_player: usize,

    /// how long an action should take before the player loses on timeout
    pub action_timeout_rate: std::time::Duration,

    /// the time after a match ends to auto start the next match
    pub auto_start_match: Option<std::time::Duration>,

    /// the time after a round ends to auto start the next round
    pub auto_start_round: Option<std::time::Duration>,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
            n_cards_per_player: 7,
            action_timeout_rate: std::time::Duration::from_secs(10),
            auto_start_match: Some(std::time::Duration::from_secs(10)),
            auto_start_round: Some(std::time::Duration::from_secs(3)),
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
    /// See [`LobbySettings.n_cards_per_player`]
    pub n_cards_per_player: usize,
    /// See [`LobbySettings.action_timeout_rate`]
    pub action_timeout_rate: std::time::Duration,
    /// See [`LobbySettings.auto_start_match`]
    pub auto_start_match: Option<std::time::Duration>,
    /// See [`LobbySettings.auto_start_round`]
    pub auto_start_round: Option<std::time::Duration>,
}

impl ActiveGameSettings {
    /// returns true if it was updated
    pub fn update(&mut self, new_settings: LobbySettings) -> bool {
        let mut modified = false;
        if self.expected_error_reaction_time != new_settings.expected_error_reaction_time {
            self.expected_error_reaction_time = new_settings.expected_error_reaction_time;
            modified = true;
        }
        if self.cards_removed_at_correct_error_report
            != new_settings.cards_removed_at_correct_error_report
        {
            self.cards_removed_at_correct_error_report =
                new_settings.cards_removed_at_correct_error_report;
            modified = true;
        }
        if self.max_cards_picked_up_when_losing != new_settings.max_cards_picked_up_when_losing {
            self.max_cards_picked_up_when_losing = new_settings.max_cards_picked_up_when_losing;
            modified = true;
        }
        if self.n_cards_per_player != new_settings.n_cards_per_player {
            self.n_cards_per_player = new_settings.n_cards_per_player;
            modified = true;
        }
        if self.action_timeout_rate != new_settings.action_timeout_rate {
            self.action_timeout_rate = new_settings.action_timeout_rate;
            modified = true;
        }
        if self.auto_start_match != new_settings.auto_start_match {
            self.auto_start_match = new_settings.auto_start_match;
            modified = true;
        }
        if self.auto_start_round != new_settings.auto_start_round {
            self.auto_start_round = new_settings.auto_start_round;
            modified = true;
        }
        modified
    }
}

impl From<&LobbySettings> for ActiveGameSettings {
    fn from(value: &LobbySettings) -> Self {
        Self {
            expected_error_reaction_time: value.expected_error_reaction_time,
            cards_removed_at_correct_error_report: value.cards_removed_at_correct_error_report,
            max_cards_picked_up_when_losing: value.max_cards_picked_up_when_losing,
            n_cards_per_player: value.n_cards_per_player,
            action_timeout_rate: value.action_timeout_rate,
            auto_start_match: value.auto_start_match,
            auto_start_round: value.auto_start_round,
        }
    }
}

pub struct GameState {
    pub settings: ActiveGameSettings,
    pub current_match: Option<MatchState>,
    pub previous_matches: Vec<MatchState>,
    pub rule_manager: RuleManager,
    lobby_tx: UnboundedSender<InternalLobbyMessage>,
}
impl GameState {
    pub fn new(settings: &LobbySettings, lobby_tx: UnboundedSender<InternalLobbyMessage>) -> Self {
        let settings = ActiveGameSettings::from(settings);
        Self {
            settings,
            current_match: None,
            previous_matches: Vec::new(),
            rule_manager: RuleManager::new(),
            lobby_tx,
        }
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
                    let public_card_stacks = current_round.public_card_stacks.clone();

                    current_round.round_termination = Some(round_termination.clone());
                    current_match
                        .previous_rounds
                        .push(current_match.current_round.take().unwrap());
                    broadcaster.broadcast(ServerMessage::RoundEnded(round_termination.clone()));
                    let guilty = match round_termination {
                        RoundTerminationType::ErrorReported { reporter, errors } => {
                            if self.settings.cards_removed_at_correct_error_report > 0 {
                                let cards = current_match.players.take_cards(
                                    &reporter,
                                    self.settings.cards_removed_at_correct_error_report,
                                );
                                if !cards.is_empty() {
                                    broadcaster.broadcast(
                                        ServerMessage::PlayerGotRidOfCardsToPool {
                                            player_id: reporter,
                                            n_cards: cards.len(),
                                        },
                                    );
                                    current_match.card_pool.add_cards(cards.into_iter());
                                }
                            }
                            errors.last().unwrap().player
                        }
                        RoundTerminationType::FaultyErrorReport(player_id) => player_id,
                        RoundTerminationType::HitPileLast(player_id) => player_id,
                        RoundTerminationType::FaultyWinDeclaration(player_id) => player_id,
                        RoundTerminationType::PlayerWonMatch(player_id) => {
                            current_match.winner = Some(player_id);
                            self.previous_matches
                                .push(self.current_match.take().unwrap());
                            broadcaster.broadcast(ServerMessage::MatchEnded);
                            if let Ok(rule_info) = self.rule_manager.add_rule() {
                                broadcaster.broadcast(ServerMessage::RuleAdded(rule_info));
                            }
                            if let Some(duration) = self.settings.auto_start_match {
                                let lobby_tx = self.lobby_tx.clone();
                                tokio::spawn(async move {
                                    tokio::time::sleep(duration).await;
                                    lobby_tx.send(InternalLobbyMessage::AutoMessage(
                                        AutoMessage::AutoStartMatch,
                                    ));
                                });
                            }
                            return;
                        }
                    };
                    let mut revealed_cards: Vec<Card> = public_card_stacks
                        .into_iter()
                        .flat_map(|card_stack| card_stack.1.into_iter())
                        .collect();
                    revealed_cards.shuffle(&mut rand::rng());
                    let picked_up_cards = match self.settings.max_cards_picked_up_when_losing {
                        MaxCardsPickedUpWhenLosing::Finite(n) => {
                            revealed_cards.split_off(revealed_cards.len().saturating_sub(n))
                        }
                        MaxCardsPickedUpWhenLosing::Unlimited => {
                            std::mem::take(&mut revealed_cards)
                        }
                    };
                    broadcaster.broadcast(ServerMessage::PlayerPickedUpRevealedCards {
                        player_id,
                        n_cards: picked_up_cards.len(),
                    });
                    current_match
                        .players
                        .add_cards_to_hand(&guilty, picked_up_cards);
                    current_match
                        .card_pool
                        .add_cards(revealed_cards.into_iter());

                    if let Some(duration) = self.settings.auto_start_round {
                        let lobby_tx = self.lobby_tx.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(duration).await;
                            lobby_tx.send(InternalLobbyMessage::AutoMessage(
                                AutoMessage::AutoStartRound,
                            ));
                        });
                    }

                    return;
                }
            }
            GameMessage::AddNewRule => todo!(),
            GameMessage::RemoveRule(_) => todo!(),
        }
    }

    pub fn handle_auto_message(&mut self, message: AutoMessage) {
        match message {
            AutoMessage::ActionTimeout => todo!(),
            AutoMessage::AutoStartMatch => todo!(),
            AutoMessage::AutoStartRound => todo!(),
        }
    }

    pub fn lobby_settings_updated(
        &mut self,
        lobby_settings: LobbySettings,
    ) -> Option<ActiveGameSettings> {
        if self.settings.update(lobby_settings) {
            Some(self.settings.clone())
        } else {
            None
        }
    }

    pub fn info(&self) -> GameInfo {
        GameInfo {
            settings: self.settings.clone(),
            current_match: self
                .current_match
                .as_ref()
                .map(|current_match| current_match.info()),
            last_match_winner: self
                .previous_matches
                .last()
                .map(|state| state.winner.unwrap()),
            active_rules: self.rule_manager.active_rules(),
        }
    }
}

// criterias use this to determine whether to launch a RuleEffect,
// and RuleEffects also use this when running
pub struct GameContext<'a> {
    pub players: &'a Vec<PlayerId>,
    pub n_cards_in_pool: usize,
    pub previous_matches: &'a Vec<MatchState>,
    pub previous_rounds: &'a Vec<RoundState>,
    pub round_starting_player: &'a PlayerId,
    /// every move/hit that is made is pushed onto this stack, including the last one
    pub player_actions: &'a Vec<PlayerAction>,
    /// the cards each player has revealed this round
    pub public_card_stacks: &'a HashMap<PlayerId, Vec<Card>>,
}

impl<'a> GameContext<'a> {
    pub fn get_just_revealed_card(&self) -> Option<&Card> {
        if let Some(PlayerAction { r#type, .. }) = self.player_actions.last()
            && let PlayerActionType::Move(player_move) = r#type
            && let PlayerMove::CountAndLayCard { card, .. } | PlayerMove::LayCard(card) =
                player_move
        {
            Some(card)
        } else {
            None
        }
    }
    pub fn get_just_said_count(&self) -> Option<&Count> {
        if let Some(PlayerAction { r#type, .. }) = self.player_actions.last()
            && let PlayerActionType::Move(player_move) = r#type
            && let PlayerMove::CountAndLayCard { count, .. } | PlayerMove::Count(count) =
                player_move
        {
            Some(count)
        } else {
            None
        }
    }
}

// fetched on request
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub settings: ActiveGameSettings,
    pub current_match: Option<MatchInfo>,
    pub last_match_winner: Option<PlayerId>,
    pub active_rules: Vec<RuleInfo>,
}

//these are internal messages
pub enum GameMessage {
    ActionPerformed(InputPlayerAction),
    AddNewRule,
    RemoveRule(usize),
}
