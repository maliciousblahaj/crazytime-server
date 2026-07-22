use crate::{
    card::{Card, CardPool},
    game::{
        PlayerId,
        round::{RoundInfo, RoundState},
    },
    lobby::LobbyPlayers,
};
use serde::Serialize;
use std::collections::HashMap;

pub struct MatchState {
    pub card_pool: CardPool,
    pub players: MatchPlayers,
    pub previous_rounds: Vec<RoundState>,
    pub current_round: Option<RoundState>,
    // if the round is finished
    pub winner: Option<PlayerId>,
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
            players: MatchPlayers::from_player_hands(player_hands),
            previous_rounds: Vec::new(),
            current_round: None,
            winner: None,
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

    pub fn handle_action_timeout(&self) {
        todo!()
    }
}

/// is sent when a new match starts, or a connection is aquired to a lobby with an existing match
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchInfo {
    players: Vec<(PlayerId, usize)>,
    n_cards_in_pool: usize,
    current_round: Option<RoundInfo>,
}

#[derive(Default)]
pub struct MatchPlayers {
    /// all match players in order
    players: Vec<PlayerId>,
    hands: HashMap<PlayerId, Vec<Card>>,
}

impl MatchPlayers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn from_player_hands(hands: HashMap<PlayerId, Vec<Card>>) -> Self {
        Self {
            players: hands.keys().copied().collect(),
            hands,
        }
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
