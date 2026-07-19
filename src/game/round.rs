use crate::{
    ServerMessage,
    card::{Card, InputTime},
    lobby::PlayerId,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::mpsc::SendError,
};
use tokio::sync::mpsc::UnboundedSender;

pub struct RoundState {
    pub init_round_state: InitRoundState,

    /// every move/hit that is made is pushed onto this stack
    pub player_actions: Vec<PlayerAction>,

    /// the cards each player has revealed this round
    pub public_card_stacks: HashMap<PlayerId, Vec<Card>>,

    /// the index of player_actions when it occured, and the reason
    pub error_occured: Option<(usize, PlayerLostReason)>,
}

// maybe we can think of rules having RuleEffect's, where some rules only have the effect on the next move
// and then forfeit their control, and some rules keep having effects for the rest of the game. Now effects
// are perfectly compatible with double rules, since double rules is only regarding rule activation (which
// launches effects on the game), not existing effects. this makes it possible for things like "at the rest
// of the round, say anything but what you're supposed to say". Now a rule effect can take in an ActionChain
// and return an ActionChain after it's been run (like changing the next player or move or whatever), but
// since multiple rule effects can be active at the same time, there has to be an order to run them in. i
// know for example, that the "chaos rule" of saying anything except what you're supposed to say would run
// absolutely last of all rule effects. like maybe there's a rule effect first which determines that the
// next player should say "1 o'clock". The chaos rule should run after this, and invert the fixed correct
// move to instead be the set of moves that are not 1 o'clock instead. also let's say the move chain is
// just fully deterministic but the chaos rule is active. if so the rule would still see what the supposed
// deterministic action is, but insert a fixed move set in front of it that inverts it, and continue doing
// this constantly (since the effect lasts forever)

pub struct DeterministicMoveChain(CountInterval);
pub struct DeterministicPlayerChain(PlayerDirection);

pub enum MoveChain {
    Deterministic(DeterministicMoveChain),
    FixedAndThen {
        // the hashset is if there are multiple possible valid moves
        moves: Vec<HashSet<PlayerMove>>,
        then: DeterministicMoveChain,
    },
}
pub enum PlayerChain {
    Deterministic(DeterministicPlayerChain),
    FixedAndThen {
        players: Vec<HashSet<PlayerId>>,
        then: DeterministicPlayerChain,
    },
}

/// The correct chain of actions in a state
pub enum ActionChain {
    Moves {
        move_chain: MoveChain,
        player_chain: PlayerChain,
    },
    Hit {
        players: Vec<PlayerId>,
        hit_type: HitType,
    },
    ReportError {
        error_player: PlayerId,
        first_error_occured: DateTime<Utc>,
        chain_before_error: Box<ActionChain>,
    },
}

enum ActionChainAdvancement {
    Continue,
    PlayerLostRound(PlayerId),
    PlayerWonMatch(PlayerId),
}

// maybe the advancement should be a rule effect in and of itself, that is inserted first out of all
// rule effects. Maybe they should be called purely RoundEffect or Effect instead, and runs all the
// time. And they only need to calculate the next move, as it will be always be run guaranteeing a
// next move even if no other effects are active. What speaks against this idea is that the advancement
// rule effect would store data that other effects should be able to modify. of course we could solve
// this by letting the effect stack as it's run make each effect insert some state that others can
// read and update, and after all are processed we run the effects in order. this would enable more
// flexibility in making really advanced meta rules.
//
// so the base effect would declare 2 fields and their current value, the next effect would maybe declare
// another one and modify the previous two, and then the next effect would not declare anything and modify
// some previous one. now all the arguments are set in stone. so the first effect has gotten the updated
// argument, and runs, and so on.
//
// but this generalization will only be useful if effects should be able to declare parameters which other
// effects can modify, and none of the existing crazytime rules require this high degree of flexibility,
// so i will skip this generalization for now and leave this comment here. For now we assume only the base
// rule effect can declare fields, with the fields being hardcoded, and represent it hardcoded as well,
// and not as an effect.

impl ActionChain {
    // returns Some if the round terminates, with who made the most error
    pub fn advance(&mut self, action: &PlayerAction) -> Option<PlayerId> {
        match action.r#type {
            PlayerActionType::Move(player_move) => {}
            PlayerActionType::Hit(hit_type) => todo!(),
            PlayerActionType::ReportError => match self {
                ActionChain::ReportError {
                    error_player,
                    first_error_occured,
                    chain_before_error,
                } => Some(error_player),
                // these should either let you go free if game settings reaction time allows it
                // or make you the player who made the worst error
                //
                // and if someone makes a wrong move, the previous chain will stay as is, like that person
                // is the one who should've made a move, so if you make a "right" move after a wrong move,
                // even if the time has gone, your move was wrong because it wasn't that player doing their
                // correct move, but you doing a move instead out of your turn
                ActionChain::Moves {
                    move_chain,
                    player_chain,
                } => todo!(),
                ActionChain::Hit { players, hit_type } => todo!(),
            },
            // should happen during a move where that player is ruled in turn, and also terminates
            PlayerActionType::DeclareWin => todo!(),
        }
    }
}

pub enum NextAction {
    Move(PlayerMove),
    EveryoneShouldHit(HitType),
}

pub enum Players {
    Players(Vec<PlayerId>),
    Everyone,
}

pub trait ActiveRule {
    // returns whether the rule stopped being active
    fn run(player: PlayerId, expected_action: &mut PlayerActionType) -> bool {
        todo!()
    }
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
#[serde(rename_all = "camelCase")]
pub struct RoundInfo {
    init_state: InitRoundState,
    latest_player_actions: Vec<(PlayerId, PlayerAction)>,
    public_card_stacks: HashMap<PlayerId, Vec<Card>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitRoundState {
    starting_player: PlayerId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoundMessage {
    ActionPerformed {
        player: PlayerId,
        action: InputPlayerAction,
    },
}

pub struct MutableRoundState<'a> {
    pub next_players: &'a mut PlayerId,
    pub next_count: &'a mut Count,

    pub direction: &'a mut PlayerDirection,
    pub count_interval: &'a mut CountInterval,

    pub should_lay_no_card: &'a mut MoveRuleApplication,
    pub should_count_the_name_of_this_rule: &'a mut MoveRuleApplication,
    pub should_count_anything_but_the_correct_count: &'a mut MoveRuleApplication,

    pub everyone_should_hit: &'a mut Option<HitType>,
}

/// in half an hour steps
/// supports negative numbers for backwards counting
pub struct CountInterval(pub isize);
impl CountInterval {
    pub const MINUSTWOHOURS: Self = Self(-4);
    pub const MINUSONEHOUR: Self = Self(-2);
    pub const MINUSTHIRTYMINUTES: Self = Self(-1);
    pub const THIRTYMINUTES: Self = Self(1);
    pub const ONEHOUR: Self = Self(2);
    pub const TWOHOURS: Self = Self(4);

    pub fn toggle_direction(&mut self) {
        self.0 = -self.0
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlayerMove {
    CountAndLayCard { card: Card, count: Count },
    Count(Count),
    LayCard(Card),
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputPlayerMove {
    CountAndLayCard(Count),
    Count(Count),
    LayCard,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputPlayerAction {
    Move(InputPlayerMove),
    Hit(HitType),
    ReportError,
    DeclareWin,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerAction {
    pub player_id: PlayerId,
    pub time: DateTime<Utc>,
    pub r#type: PlayerActionType,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlayerActionType {
    Move(PlayerMove),
    Hit(HitType),
    ReportError,
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

pub enum PlayerDirection {
    Forward,
    Reverse,
}

impl PlayerDirection {
    pub fn toggle(&mut self) {
        *self = match self {
            Self::Forward => Self::Reverse,
            Self::Reverse => Self::Forward,
        };
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HitType {
    // hit with right hand
    Single,
    // hit with both hands
    Double,
    // hit with right hand upside down
    UpsideDown,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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
