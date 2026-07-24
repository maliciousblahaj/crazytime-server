use crate::{
    ServerMessage,
    card::{Card, CardPool},
    game::{
        ActiveGameSettings, MaxCardsPickedUpWhenLosing, PlayerId,
        round::{RoundInfo, RoundState, RoundTerminationType},
    },
    lobby::{LobbyBroadcaster, LobbyPlayers},
};
use rand::seq::SliceRandom;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug)]
pub struct MatchState {
    pub card_pool: CardPool,
    pub players: MatchPlayers,
    pub previous_rounds: Vec<RoundState>,
    pub current_round: Option<RoundState>,
    // if the round is finished
    pub match_termination: Option<MatchTerminationType>,
}

impl MatchState {
    /// returns Err(()) if too many cards per player
    pub fn new(lobby_players: &LobbyPlayers, n_cards_per_player: usize) -> Result<Self, ()> {
        let mut card_pool = CardPool::new();
        if card_pool.n_cards() < n_cards_per_player * lobby_players.len() {
            return Err(());
        }
        let player_hands = lobby_players
            .players()
            .map(|player| (*player, card_pool.take_cards(n_cards_per_player)))
            .collect();
        Ok(Self {
            card_pool,
            players: MatchPlayers::new(lobby_players.players().copied().collect(), player_hands),
            previous_rounds: Vec::new(),
            current_round: None,
            match_termination: None,
        })
    }

    pub fn info(&self) -> MatchInfo {
        MatchInfo {
            players: self
                .players
                .hands
                .iter()
                .map(|(player, cards)| (*player, cards.len()))
                .collect(),
            n_cards_in_pool: self.card_pool.n_cards(),
            current_round: self.current_round.as_ref().map(|round| round.info()),
        }
    }

    /// returns Some if the match terminated as well (aka if input round termination is playerwonmatch, where the
    /// match termination is already broadcasted, so don't worry about that, only remove the match from current)
    ///
    /// doesn't initiate autostartround or autostartmatch
    pub fn round_terminated(
        &mut self,
        round_termination: RoundTerminationType,
        broadcaster: &LobbyBroadcaster,
        game_settings: &ActiveGameSettings,
    ) -> bool {
        let Some(mut current_round) = self.current_round.take() else {
            return false;
        };

        current_round.round_termination = Some(round_termination.clone());
        let revealed_card_stacks = current_round.revealed_card_stacks.clone();
        self.previous_rounds.push(current_round);

        broadcaster.broadcast(ServerMessage::RoundEnded(round_termination.clone()));
        let guilty = match round_termination {
            RoundTerminationType::ErrorReported { reporter, errors } => {
                if game_settings.cards_removed_at_correct_error_report > 0 {
                    let cards = self.players.take_cards(
                        &reporter,
                        game_settings.cards_removed_at_correct_error_report,
                    );
                    if !cards.is_empty() {
                        broadcaster.broadcast(ServerMessage::PlayerGotRidOfCardsToPool {
                            player_id: reporter,
                            n_cards: cards.len(),
                        });
                        self.card_pool.add_cards(cards.into_iter());
                    }
                }
                errors.last().unwrap().player
            }
            RoundTerminationType::FaultyErrorReport(player_id) => player_id,
            RoundTerminationType::HitPileLast(player_id) => player_id,
            RoundTerminationType::FaultyWinDeclaration(player_id) => player_id,
            RoundTerminationType::Timeout(player_id) => player_id,
            RoundTerminationType::PlayerWonMatch(player_id) => {
                let match_termination = MatchTerminationType::PlayerWonMatch(player_id);
                self.match_termination = Some(match_termination.clone());
                broadcaster.broadcast(ServerMessage::MatchEnded(match_termination));
                return true;
            }
        };
        let mut revealed_cards: Vec<Card> = revealed_card_stacks
            .into_iter()
            .flat_map(|card_stack| card_stack.1.into_iter())
            .collect();
        revealed_cards.shuffle(&mut rand::rng());
        let picked_up_cards = match game_settings.max_cards_picked_up_when_losing {
            MaxCardsPickedUpWhenLosing::Finite(n) => {
                revealed_cards.split_off(revealed_cards.len().saturating_sub(n))
            }
            MaxCardsPickedUpWhenLosing::Unlimited => std::mem::take(&mut revealed_cards),
        };
        broadcaster.broadcast(ServerMessage::PlayerPickedUpRevealedCards {
            player_id: guilty,
            n_cards: picked_up_cards.len(),
        });
        self.players.add_cards_to_hand(&guilty, picked_up_cards);
        self.card_pool.add_cards(revealed_cards.into_iter());

        false
    }
}

/// is sent when a new match starts, or a connection is aquired to a lobby with an existing match
#[derive(Clone, Serialize)]
pub struct MatchInfo {
    players: Vec<(PlayerId, usize)>,
    n_cards_in_pool: usize,
    current_round: Option<RoundInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub enum MatchTerminationType {
    PlayerWonMatch(PlayerId),
    /// happens if someone leaves and the lobby ends up with fewer than 3 people, or if the host cancels
    MatchCancelled,
}

#[derive(Debug, Default)]
pub struct MatchPlayers {
    /// all match players in order
    players: Vec<PlayerId>,
    hands: HashMap<PlayerId, Vec<Card>>,
}

impl MatchPlayers {
    pub fn new(player_order: Vec<PlayerId>, hands: HashMap<PlayerId, Vec<Card>>) -> Self {
        if hands.len() != player_order.len() {
            // panics here because this is a critical bug if it ever gets to this point
            panic!("matchplayers parameter invariant broken!!");
        }
        for player in player_order.iter() {
            // panics here because this is a critical bug if it ever gets to this point
            if !hands.contains_key(player) {
                panic!("matchplayers parameter invariant broken!!");
            }
        }
        Self {
            players: player_order,
            hands,
        }
    }
    /// the number of players
    pub fn n_players(&self) -> usize {
        self.players.len()
    }

    pub fn get_player_vec(&self) -> &Vec<PlayerId> {
        &self.players
    }

    pub fn get(&self, index: usize) -> Option<&PlayerId> {
        self.players.get(index)
    }

    pub fn get_hand(&self, player: &PlayerId) -> Option<&Vec<Card>> {
        self.hands.get(player)
    }

    /// take a card from highest up in their card stack
    pub fn take_card(&mut self, player: &PlayerId) -> Option<Card> {
        self.hands.get_mut(player).unwrap().pop()
    }

    /// take n cards from highest up in their card stack. If not enough cards exist, the ones that do still are retrieved
    pub fn take_cards(&mut self, player: &PlayerId, n_cards: usize) -> Vec<Card> {
        let card_pile = self.hands.get_mut(player).unwrap();
        let mut cards = Vec::new();
        for _ in 0..n_cards.min(card_pile.len()) {
            cards.push(card_pile.pop().unwrap());
        }
        cards
    }

    pub fn add_cards_to_hand(&mut self, player: &PlayerId, mut cards: Vec<Card>) {
        let card_pile = self.hands.get_mut(player).unwrap();
        cards.append(card_pile);
        *card_pile = cards;
    }

    /// returns (false, index) if the player already exists at a certain index,
    /// else (true, index) if the player was just inserted at a certain index.
    pub fn add_player(&mut self, player: PlayerId, hand: Vec<Card>) -> (bool, usize) {
        if let Some(index) = self.players.iter().position(|i_player| *i_player == player) {
            return (false, index);
        }
        self.players.push(player);
        self.hands.insert(player, hand);
        (true, self.players.len() - 1)
    }
    /// returns their card hands if the player was removed, and None if they
    /// don't exist in the set
    pub fn remove_player(&mut self, player: &PlayerId) -> Option<Vec<Card>> {
        self.players.retain(|i_player| i_player != player);
        self.hands.remove(&player)
    }

    /// get the index of a player
    pub fn get_index(&self, player: &PlayerId) -> Option<usize> {
        self.players.iter().position(|i_player| i_player == player)
    }
}
