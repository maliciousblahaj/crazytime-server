use crate::{
    ServerMessage,
    card::{Card, InputTime},
    lobby::PlayerId,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::mpsc::SendError};
use tokio::sync::mpsc::UnboundedSender;

pub struct RoundState {
    pub init_state: InitRoundState,

    /// every move/hit that is made is pushed onto this stack
    pub player_actions: Vec<PlayerAction>,

    /// the cards each player has revealed this round
    pub public_card_stacks: HashMap<PlayerId, Vec<Card>>,

    /// NOT the same as public player id, but just the index in the players array
    pub current_player_index: usize,

    /// the index of player_actions when it occured, and the reason
    pub error_occured: Option<(usize, PlayerLostReason)>,
}

impl RoundState {
    pub fn previous_move(&self) -> Option<(&PlayerId, &PlayerMove)> {
        self.player_actions.iter().rev().find_map(
            |PlayerAction {
                 player_id, r#type, ..
             }| match r#type {
                PlayerActionType::Move(player_move) => Some((player_id, player_move)),
                _ => None,
            },
        )
    }

    // /// the previous card that was played, so if someone counts without laying a card
    // /// the one before that will still be the previously played
    // pub fn previously_played_card(&self) -> Option<Card> {
    //     for (_, player_move) in self.player_moves.iter().rev() {
    //         match player_move {
    //             PlayerMove::CountAndLayCard { card, .. } => {
    //                 return Some(*card);
    //             }
    //             PlayerMove::LayCard(card) => {
    //                 return Some(*card);
    //             }
    //             _ => {}
    //         }
    //     }
    //     None
    // }
    pub async fn handle_message(
        player: PlayerId,
        message: RoundMessage,
        tx: UnboundedSender<ServerMessage>,
    ) -> Result<(), SendError<ServerMessage>> {
        Ok(())
    }
}

#[derive(Serialize)]
pub struct RoundInfo {
    init_state: InitRoundState,
    latest_player_actions: Vec<(PlayerId, PlayerAction)>,
    public_card_stacks: HashMap<PlayerId, Vec<Card>>,
}

#[derive(Serialize, Deserialize)]
pub struct InitRoundState {
    starting_player: PlayerId,
}

pub enum RoundMessage {
    ActionPerformed {
        player: PlayerId,
        action: PlayerActionType,
    },
}

// might merge with RoundState tbh, or make it a field inside RoundState, but id have
// to figure out immutability of the relevant RoundState parameters
pub struct MutableRoundState {
    // this runs after the next player index has been incremented,
    // so it doesnt overwrite this provided value, and the same for next_count,
    // as that would allow for arithmetic like what you should've said + half an hour
    pub next_player: PlayerId,
    pub next_count: Count,

    pub direction: TurnDirection,
    pub count_interval: CountInterval,

    pub should_lay_no_card: MoveRuleApplication,
    pub should_count_the_name_of_this_rule: MoveRuleApplication,
    pub should_count_anything_but_the_correct_count: MoveRuleApplication,

    pub everyone_should_hit: Option<HitType>,
}
/// in half an hour steps
pub struct CountInterval(pub usize);
impl CountInterval {
    pub const THIRTYMINUTES: Self = Self(1);
    pub const ONEHOUR: Self = Self(2);
    pub const TWOHOURS: Self = Self(4);
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum Count {
    Time(InputTime),
    NameOfThisRule,
}

pub struct FinishedRound {
    pub player_actions: Vec<(PlayerId, PlayerAction)>,
    /// the index of player_moves in which an error occured. this is only one index,
    /// since all moves after an error are also errors, until someone points it out
    pub error_occured: Option<usize>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum PlayerMove {
    CountAndLayCard { card: Card, count: Count },
    Count(Count),
    LayCard(Card),
}

#[derive(Serialize)]
pub struct PlayerAction {
    player_id: PlayerId,
    time: DateTime<Utc>,
    r#type: PlayerActionType,
}
#[derive(Serialize, Deserialize)]
pub enum PlayerActionType {
    Move(PlayerMove),
    Hit(HitType),
    CallError,
    DeclareWin,
}
pub struct MoveRuleApplication(Option<MoveRuleApplicationInner>);
impl MoveRuleApplication {
    pub fn turn_progressed(mut self) {
        self.0 = match self.0 {
            Some(MoveRuleApplicationInner {
                target,
                validity: MoveRuleValidity::Turns(n),
            }) => {
                if n <= 1 {
                    None
                } else {
                    Some(MoveRuleApplicationInner {
                        target,
                        validity: MoveRuleValidity::Turns(n - 1),
                    })
                }
            }
            other => other,
        }
    }
}
/// discussion: so this allows for certain states, like a rule applying to all players
/// indefinitely, or for a group of players indefinitely, or a rule applying to everyone
/// for n turns, or a rule applying for a group of players for n turns, i think this all
/// makes sense. just make sure to modify validity as turns progress.
pub struct MoveRuleApplicationInner {
    target: MoveRuleTarget,
    validity: MoveRuleValidity,
}
/// who a rule is applied on
pub enum MoveRuleTarget {
    Players(Vec<PlayerId>),
    Everyone,
}
/// how long a rule is applied for
pub enum MoveRuleValidity {
    Turns(usize),
    Indefinitely,
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

#[derive(Serialize, Deserialize)]
pub enum HitType {
    // hit with right hand
    Single,
    // hit with both hands
    Double,
    // hit with right hand upside down
    UpsideDown,
}

#[derive(Serialize, Deserialize)]
pub enum PlayerLostReason {
    // if you report a move that was valid
    FaultyErrorReport,
    // if a player calls you out on doing an incorrect move
    IncorrectMove,
    // if you hit last on a pile where it you were supposed to not hit
    FaultyHitLast,
    HitLast,
    // this error takes priority over hitting last, because one cannot be expected
    // to react so quickly over a wrong hit type
    WrongHitType(HitType),
}
