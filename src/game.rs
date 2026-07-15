use crate::{
    card::{Card, CardManager, Time},
    player::{PublicPlayerId, ValidPlayerMove},
};

pub struct GameState {
    rounds: Vec<GameRound>,
}

impl GameState {
    pub fn new() -> Self {
        todo!()
    }
}

pub struct GameRound {
    pub card_manager: CardManager,
    pub round_state: AdaptiveRoundState,
}

pub struct PublicGameState {
    /// every move that is made is pushed onto this stack
    pub player_moves: Vec<(PublicPlayerId, ValidPlayerMove)>,
    /// NOT the same as public player id, but just the index in the players array
    pub current_player_index: usize,

    /// all previous rounds saved
    pub previous_rounds: Vec<FinishedRound>,
    /// all previous matches saved
    pub previous_matches: Vec<FinishedMatch>,

    pub players: Vec<PublicPlayerData>,
}

pub enum PlayerAction {
    Move(ValidPlayerMove),
    ErrorCall,
    Hit(HitType),
}

pub struct FinishedMatch {
    /// ordered placing, from first to last place. item 0 is the winner of the round
    pub placings: Vec<PublicPlayerId>,
    // pub rules: Vec
}
pub struct FinishedRound {
    pub player_moves: Vec<(PublicPlayerId, ValidPlayerMove)>,
    /// the index of player_moves in which an error occured. this is only one index,
    /// since all moves after an error are also errors, until someone points it out
    pub error_occured: Option<usize>,
    pub players: Vec<PublicPlayerData>,
}

pub struct PublicPlayerData {
    id: PublicPlayerId,
    n_cards: usize,
    revealed_card_stack: Vec<Card>,
}

impl PublicGameState {
    pub fn previous_move(&self) -> Option<ValidPlayerMove> {
        self.player_moves
            .last()
            .map(|(_, previous_move)| *previous_move)
    }

    /// the previous card that was played, so if someone counts without laying a card
    /// the one before that will still be the previously played
    pub fn previously_played_card(&self) -> Option<Card> {
        for (_, player_move) in self.player_moves.iter().rev() {
            match player_move {
                ValidPlayerMove::CountAndLayCard { card, .. } => {
                    return Some(*card);
                }
                ValidPlayerMove::LayCard(card) => {
                    return Some(*card);
                }
                _ => {}
            }
        }
        None
    }
}

pub struct AdaptiveRoundState {
    pub direction: TurnDirection,
    // this runs after the next player index has been incremented,
    // so it doesnt overwrite this provided value, and the same for next_count,
    // as that would allow for arithmetic like what you should've said + half an hour
    pub next_player_index: usize,
    pub next_count: Time,

    /// in half an hour steps
    pub count_interval_index: usize,

    // when one lays no card the ValidPlayerMove will just use the previous card in that place
    pub should_lay_no_card: bool,
    pub should_say_the_name_of_this_rule: bool,
    pub should_say_anything_but_correct_count: bool,

    pub everyone_should_hit: Option<HitType>,
    pub everyone_should_say_anything_but_correct_count: bool,
}

pub enum TurnDirection {
    Forward,
    Reverse,
}

impl TurnDirection {
    pub fn toggle(&mut self) {
        *self = match self {
            Self::Forward => Self::Reverse,
            Self::Reverse => Self::Forward,
        };
    }
}

pub enum HitType {
    Single,
    Double,
    UpsideDown,
}
