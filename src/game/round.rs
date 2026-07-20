use crate::{
    ServerMessage,
    card::{Card, Time},
    game::ActiveGameSettings,
    lobby::{LobbyBroadcaster, PlayerId},
    rules::RuleEffect,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::mpsc::SendError,
};

pub struct RoundState {
    pub init_round_state: InitRoundState,

    /// every action that is made is pushed onto this stack, to log it
    pub player_actions: Vec<PlayerAction>,

    /// the cards each player has revealed this round
    pub public_card_stacks: HashMap<PlayerId, Vec<Card>>,

    /// the active effects, inverse order, should be evaluated from last to first element
    pub active_effects: Vec<RuleEffect>,

    pub action_chain: ActionChain,
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

pub struct MoveManager {
    count_interval: TimeInterval,
    // im thinking if it might be not the best idea to make this a VecDeque, maybe it should only be
    // possible to fix one valid move in advance, where maybe we don't need this generalization. this
    // is because it is more difficult to work with since what if a fixed move is made that reveals
    // a card and suddenly another rule effect starts being in play. that rule effect would now need
    // to handle the existing possiblity of a fixed chain being there
    //
    // the hashset is for supporting multiple possible valid moves
    move_queue: VecDeque<HashSet<ValidPlayerMoveType>>,
}
impl MoveManager {
    /// returns Some if the move was correct, and None if incorrect
    pub fn process<'a>(
        &mut self,
        previous_moves: impl DoubleEndedIterator<Item = &'a ValidPlayerMoveType>,
        player_move: InputPlayerMove,
    ) -> Option<ValidPlayerMoveType> {
        let Ok(player_move) = ValidPlayerMoveType::try_from(player_move) else {
            return None;
        };
        if let Some(allowed_moves) = self.move_queue.pop_front() {
            if allowed_moves.contains(&player_move) {
                return Some(player_move);
            } else {
                return None;
            }
        }

        let mut previous_time = None;
        for prev_move in previous_moves.rev() {
            if let Some(ValidCount::Time(time)) = prev_move.get_count() {
                previous_time = Some(time);
                break;
            }
        }
        let expected_time = match previous_time {
            Some(time) => time.plus(&self.count_interval),
            None => Time::One,
        };
        match player_move {
            ValidPlayerMoveType::CountAndLayCard(valid_count) => {
                if let ValidCount::Time(time) = valid_count
                    && time == expected_time
                {
                    Some(player_move)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

pub struct PlayerManager {
    player_order: Vec<PlayerId>,
    pub direction: PlayerDirection,
    pub player_queue: VecDeque<HashSet<PlayerId>>,
}

impl PlayerManager {
    /// returns true if the player was correct, and false if incorrect
    pub fn process(&mut self, player: PlayerId, previous_player: &PlayerId) -> bool {
        if let Some(allowed_players) = self.player_queue.pop_front() {
            if allowed_players.contains(&player) {
                return true;
            } else {
                return false;
            }
        }
        let previous_player_idx = self
            .player_order
            .iter()
            .position(|player| player == previous_player)
            .unwrap();
        let next_player = self.player_order[(previous_player_idx as isize + self.direction.0)
            .rem_euclid(self.player_order.len() as isize)
            as usize];
        player == next_player
    }
}

struct PreviousMoves(Vec<ValidPlayerMove>);

/// The correct chain of actions in a state
pub enum ActionChain {
    Moves {
        previous_moves: PreviousMoves,
        player_manager: PlayerManager,
        move_manager: MoveManager,
    },
    Hit {
        // when this set becomes empty the round terminates
        players: HashSet<PlayerId>,
        hit_type: HitType,
    },
    ReportError {
        errors: Vec<ActionError>,
        chain_before_first_error: Box<ActionChain>,
    },
}

pub struct ActionError {
    player: PlayerId,
    reason: ErrorReason,
    occured: DateTime<Utc>,
}

enum ErrorReason {
    InvalidMove,
    OutOfTurn,
    InvalidHit,
    InvalidHitType,
    InvalidWinDeclaration,
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

pub enum RoundTerminationType {
    ErrorReported {
        reporter: PlayerId,
        errors: Vec<ActionError>,
    },
    FaultyErrorReport(PlayerId),
    HitPileLast(PlayerId),
}

// pub enum ProcessedAction {
//     Continue(ActionChain),
//     Termination(RoundTerminationType),
// }

impl ActionChain {
    // returns Some if the round terminates, with the list of people who made errors
    pub fn process_action(
        &mut self,
        player_id: PlayerId,
        settings: &ActiveGameSettings,
        action: InputPlayerAction,
        time: DateTime<Utc>,
    ) -> Option<RoundTerminationType> {
        let chain = if let ActionChain::ReportError {
            errors,
            chain_before_first_error,
        } = self
            && (time - errors.first().unwrap().occured) <= settings.expected_error_reaction_time
        {
            chain_before_first_error
        } else {
            self
        };
        // if i were to not take self by value, but a mut reference, i could here instead match
        // on self first to catch the edge case of valid move after error within error reaction time,
        // and not have to duplicate later
        match action {
            InputPlayerAction::Move(player_move) => {
                if let ActionChain::Moves {
                    previous_moves,
                    player_manager,
                    move_manager,
                } = if let ActionChain::ReportError {
                    errors,
                    chain_before_first_error,
                } = self
                    && (time - errors.first().unwrap().occured)
                        <= settings.expected_error_reaction_time
                {
                    *chain_before_first_error
                } else {
                    self
                } {
                    match move_manager.process(
                        previous_moves.0.iter().map(|prev_move| &prev_move.r#type),
                        player_move,
                    ) {
                        Some(valid_move_type) => {
                            previous_moves.0.push(ValidPlayerMove {
                                player: player_id,
                                r#type: valid_move_type,
                            });
                            return ProcessedAction::Continue(self);
                        }
                        None => {
                            let error = ActionError {
                                player: player_id,
                                reason: ErrorReason::InvalidMove,
                                occured: time,
                            };
                            return ProcessedAction::Continue(Self::ReportError {
                                errors: Vec::from([error]),
                                chain_before_first_error: Box::new(self),
                            });
                        }
                    }
                }
                if let ActionChain::ReportError {
                    errors,
                    chain_before_first_error,
                } = self
                {
                    errors.push(ActionError {
                        player: player_id,
                        reason: ErrorReason::OutOfTurn,
                        occured: time,
                    });
                    return ProcessedAction::Continue(Self::ReportError {
                        errors,
                        chain_before_first_error,
                    });
                }
                ProcessedAction::Continue(Self::ReportError {
                    errors: Vec::from([ActionError {
                        player: player_id,
                        reason: ErrorReason::OutOfTurn,
                        occured: time,
                    }]),
                    chain_before_first_error: Box::new(self),
                })
            }
            InputPlayerAction::Hit(ref hit_type) => {
                let Self::Hit { players, hit_type } = self else {
                    replace_with::replace_with_or_abort(self, |prev_self| Self::ReportError {
                        errors: Vec::from([ActionError {
                            player: player_id,
                            reason: ErrorReason::InvalidHit,
                            occured: time,
                        }]),
                        chain_before_first_error: Box::new(prev_self),
                    });
                    return None;
                };

                if !players.remove(&player_id) {
                    replace_with::replace_with_or_abort(self, |prev_self| Self::ReportError {
                        errors: Vec::from([ActionError {
                            player: player_id,
                            reason: ErrorReason::InvalidHit,
                            occured: time,
                        }]),
                        chain_before_first_error: Box::new(prev_self),
                    });
                }
                if players.is_empty() {
                    // this runs only if this was the last player to hit
                    Some(RoundTerminationType::HitPileLast(player_id))
                } else {
                    None
                }
            }
            InputPlayerAction::ReportError => match self {
                Self::ReportError {
                    errors,
                    chain_before_first_error,
                } => Some(RoundTerminationType::ErrorReported {
                    reporter: player_id,
                    errors,
                }),
                _ => Some(RoundTerminationType::FaultyErrorReport(player_id)),
            },
            // should happen during a move where that player is ruled in turn, and also terminates
            InputPlayerAction::DeclareWin => todo!(),
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
        &mut self,
        player: PlayerId,
        message: RoundMessage,
        broadcaster: &LobbyBroadcaster,
    ) -> Result<(), SendError<ServerMessage>> {
        match message {
            RoundMessage::ActionPerformed(input_player_action) => todo!(),
            RoundMessage::MoveTimeout => todo!(),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundInfo {
    init_state: InitRoundState,
    latest_player_actions: Vec<(PlayerId, PlayerAction)>,
    public_card_stacks: HashMap<PlayerId, Vec<Card>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitRoundState {
    starting_player: PlayerId,
}

//these are internal messages
pub enum RoundMessage {
    ActionPerformed(InputPlayerAction),
    MoveTimeout,
}

pub struct MutableRoundState<'a> {
    pub next_players: &'a mut PlayerId,
    pub next_count: &'a mut Count,

    pub direction: &'a mut PlayerDirection,
    pub time_interval: &'a mut TimeInterval,

    pub should_lay_no_card: &'a mut MoveRuleApplication,
    pub should_count_the_name_of_this_rule: &'a mut MoveRuleApplication,
    pub should_count_anything_but_the_correct_count: &'a mut MoveRuleApplication,

    pub everyone_should_hit: &'a mut Option<HitType>,
}

/// in half an hour steps
/// supports negative numbers for backwards counting
#[derive(Copy, Clone)]
pub struct TimeInterval(pub isize);
impl TimeInterval {
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

#[derive(Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Count {
    Zero,
    ZeroThirty,
    One,
    OneThirty,
    Two,
    TwoThirty,
    Three,
    ThreeThirty,
    Four,
    FourThirty,
    Five,
    FiveThirty,
    Six,
    SixThirty,
    Seven,
    SevenThirty,
    Eight,
    EightThirty,
    Nine,
    NineThirty,
    Ten,
    TenThirty,
    Eleven,
    ElevenThirty,
    Twelve,
    TwelveThirty,
    Thirteen,
    ThirteenThirty,
    Fourteen,
    FourteenThirty,
    Fifteen,
    FifteenThirty,
    Sixteen,
    SixteenThirty,
    Seventeen,
    SeventeenThirty,
    Eighteen,
    EighteenThirty,
    Nineteen,
    NineteenThirty,
    NameOfThisRule,
}
impl From<Time> for Count {
    fn from(value: Time) -> Self {
        match value {
            Time::One => Self::One,
            Time::OneThirty => Self::OneThirty,
            Time::Two => Self::Two,
            Time::TwoThirty => Self::TwoThirty,
            Time::Three => Self::Three,
            Time::ThreeThirty => Self::ThreeThirty,
            Time::Four => Self::Four,
            Time::FourThirty => Self::FourThirty,
            Time::Five => Self::Five,
            Time::FiveThirty => Self::FiveThirty,
            Time::Six => Self::Six,
            Time::SixThirty => Self::SixThirty,
            Time::Seven => Self::Seven,
            Time::SevenThirty => Self::SevenThirty,
            Time::Eight => Self::Eight,
            Time::EightThirty => Self::EightThirty,
            Time::Nine => Self::Nine,
            Time::NineThirty => Self::NineThirty,
            Time::Ten => Self::Ten,
            Time::TenThirty => Self::TenThirty,
            Time::Eleven => Self::Eleven,
            Time::ElevenThirty => Self::ElevenThirty,
            Time::Twelve => Self::Twelve,
            Time::TwelveThirty => Self::TwelveThirty,
        }
    }
}

pub struct FinishedRound {
    pub player_actions: Vec<(PlayerId, PlayerAction)>,
    /// the index of player_moves in which an error occured. this is only one index,
    /// since all moves after an error are also errors, until someone points it out
    pub error_occured: Option<usize>,
}

// same as InputPlayerMove but with cards revealed, and broadcast capabilities
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlayerMove {
    CountAndLayCard { card: Card, count: Count },
    Count(Count),
    LayCard(Card),
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputPlayerAction {
    Move(InputPlayerMove),
    Hit(HitType),
    ReportError,
    DeclareWin,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputPlayerMove {
    CountAndLayCard(Count),
    Count(Count),
    LayCard,
}

pub struct ValidPlayerMove {
    player: PlayerId,
    r#type: ValidPlayerMoveType,
}

#[derive(PartialEq, Eq, Hash)]
pub enum ValidPlayerMoveType {
    CountAndLayCard(ValidCount),
    Count(ValidCount),
    LayCard,
}
impl TryFrom<InputPlayerMove> for ValidPlayerMoveType {
    type Error = ();

    fn try_from(value: InputPlayerMove) -> Result<Self, Self::Error> {
        Ok(match value {
            InputPlayerMove::CountAndLayCard(count) => {
                ValidPlayerMoveType::CountAndLayCard(ValidCount::try_from(count)?)
            }
            InputPlayerMove::Count(count) => {
                ValidPlayerMoveType::Count(ValidCount::try_from(count)?)
            }
            InputPlayerMove::LayCard => ValidPlayerMoveType::LayCard,
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub enum ValidCount {
    Time(Time),
    NameOfThisRule,
}

impl TryFrom<Count> for ValidCount {
    type Error = ();

    fn try_from(value: Count) -> Result<Self, Self::Error> {
        Ok(match value {
            Count::NameOfThisRule => Self::NameOfThisRule,
            other => Self::Time(Time::try_from(other)?),
        })
    }
}

impl ValidPlayerMoveType {
    pub fn get_count(&self) -> Option<&ValidCount> {
        match self {
            Self::CountAndLayCard(count) => Some(count),
            Self::Count(count) => Some(count),
            Self::LayCard => None,
        }
    }
}

impl InputPlayerMove {
    pub fn get_count(&self) -> Option<Count> {
        match self {
            InputPlayerMove::CountAndLayCard(count) => Some(*count),
            InputPlayerMove::Count(count) => Some(*count),
            InputPlayerMove::LayCard => None,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerAction {
    pub player_id: PlayerId,
    pub time: DateTime<Utc>,
    pub r#type: PlayerActionType,
}

#[derive(Clone, Serialize)]
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

/// the index offset to use
#[derive(Copy, Clone)]
pub struct PlayerDirection(pub isize);

impl PlayerDirection {
    pub const FORWARD: Self = Self(1);
    pub const REVERSE: Self = Self(-1);

    pub fn toggle_direction(&mut self) {
        self.0 = -self.0
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HitType {
    // hit with right hand
    Single,
    // hit with both hands
    Double,
    // hit with right hand upside down
    UpsideDown,
}

#[derive(Clone, Serialize)]
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
