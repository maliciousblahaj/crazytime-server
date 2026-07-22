use crate::{
    card::{Card, Time},
    game::{ActiveGameSettings, r#match::MatchPlayers},
    lobby::PlayerId,
    rules::RuleEffect,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub struct RoundState {
    pub starting_player: PlayerId,

    /// every action that is made is pushed onto this stack, to log it, including the terminating action
    pub player_actions: Vec<PlayerAction>,

    /// the cards each player has revealed this round
    pub revealed_card_stacks: HashMap<PlayerId, Vec<Card>>,

    /// the active effects, inverse order, should be evaluated from last to first element
    pub active_effects: Vec<RuleEffect>,

    pub action_chain: ActionChain,

    // for when a round is finished
    pub round_termination: Option<RoundTerminationType>,
}

impl RoundState {
    pub fn new(starting_player: PlayerId) -> Self {
        Self {
            starting_player,
            player_actions: Vec::new(),
            revealed_card_stacks: HashMap::new(),
            active_effects: Vec::new(),
            action_chain: ActionChain::Moves {
                previous_moves: Vec::new(),
                turn_manager: TurnManager {
                    starting_player,
                    direction: PlayerDirection::FORWARD,
                    next_player: None,
                },
                move_manager: MoveManager {
                    count_interval: TimeInterval::ONEHOUR,
                    next_move: None,
                },
            },
            round_termination: None,
        }
    }

    pub fn info(&self) -> RoundInfo {
        RoundInfo {
            starting_player: self.starting_player,
            player_actions: self.player_actions.clone(),
            revealed_card_stacks: self.revealed_card_stacks.clone(),
        }
    }

    pub fn get_previous_moves(&self) -> Vec<(PlayerId, PlayerMove)> {
        self.player_actions
            .iter()
            .rev()
            .filter_map(|action| match action.r#type {
                PlayerActionType::Move(player_move) => Some((action.player_id, player_move)),
                _ => None,
            })
            .collect()
    }
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
    pub count_interval: TimeInterval,
    // im thinking if it might be not the best idea to make this a VecDeque, maybe it should only be
    // possible to fix one valid move in advance, where maybe we don't need this generalization. this
    // is because it is more difficult to work with since what if a fixed move is made that reveals
    // a card and suddenly another rule effect starts being in play. that rule effect would now need
    // to handle the existing possiblity of a fixed chain being there
    //
    // the hashset is for supporting multiple possible valid moves
    pub next_move: Option<HashSet<ValidInputPlayerMoveType>>,
}
impl MoveManager {
    /// returns Some if the move was correct, and None if incorrect
    pub fn process<'a>(
        &mut self,
        previous_moves: impl DoubleEndedIterator<Item = &'a ValidInputPlayerMoveType>,
        player_move: InputPlayerMove,
    ) -> Option<ValidInputPlayerMoveType> {
        let Ok(player_move) = ValidInputPlayerMoveType::try_from(player_move) else {
            return None;
        };
        if let Some(allowed_moves) = self.next_move.take() {
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
            ValidInputPlayerMoveType::CountAndLayCard(count) => {
                if let ValidCount::Time(time) = count
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

    pub fn set_next_move(&mut self, valid_move: ValidInputPlayerMoveType) {
        self.next_move = Some(HashSet::from([valid_move]));
    }
}

pub struct TurnManager {
    starting_player: PlayerId,
    pub direction: PlayerDirection,
    pub next_player: Option<HashSet<PlayerId>>,
}

impl TurnManager {
    /// returns true if the player was correct, and false if incorrect
    pub fn process(
        &mut self,
        player: PlayerId,
        previous_player: Option<&PlayerId>,
        match_players: &MatchPlayers,
    ) -> bool {
        if let Some(allowed_players) = self.next_player.take() {
            if allowed_players.contains(&player) {
                return true;
            } else {
                return false;
            }
        }
        let next_player = if let Some(previous_player) = previous_player {
            let previous_player_idx = match_players.get_index(previous_player).unwrap();

            *match_players
                .get(
                    (previous_player_idx as isize + self.direction.0)
                        .rem_euclid(match_players.len() as isize) as usize,
                )
                .unwrap()
        } else {
            self.starting_player
        };

        player == next_player
    }
    pub fn set_next_player(&mut self, player: PlayerId) {
        self.next_player = Some(HashSet::from([player]));
    }
}

/// provided as input for rules, made from destructuring ActionChain
pub struct ActionChainMoves {
    pub previous_moves: Vec<ValidInputPlayerMove>,
    pub turn_manager: TurnManager,
    pub move_manager: MoveManager,
}

/// The correct chain of actions in a state
pub enum ActionChain {
    Moves {
        previous_moves: Vec<ValidInputPlayerMove>,
        turn_manager: TurnManager,
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionError {
    pub player: PlayerId,
    pub reason: ErrorReason,
    pub occured: DateTime<Utc>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RoundTerminationType {
    ErrorReported {
        reporter: PlayerId,
        // latest error maker in this vec is the loser of the round
        errors: Vec<ActionError>,
    },
    FaultyErrorReport(PlayerId),
    HitPileLast(PlayerId),
    FaultyWinDeclaration(PlayerId),
    PlayerWonMatch(PlayerId),
}
impl RoundTerminationType {
    pub fn get_loser(&self) -> Option<&PlayerId> {
        match self {
            RoundTerminationType::ErrorReported { errors, .. } => {
                Some(&errors.last().unwrap().player)
            }
            RoundTerminationType::FaultyErrorReport(player_id) => Some(player_id),
            RoundTerminationType::HitPileLast(player_id) => Some(player_id),
            RoundTerminationType::FaultyWinDeclaration(player_id) => Some(player_id),
            RoundTerminationType::PlayerWonMatch(_) => None,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorReason {
    MoveOutOfTurn,
    InvalidMove,
    InvalidHit,
    InvalidHitType,
    InvalidWinDeclaration,
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

/// checks if self is ReportError and adds to its errors, else makes self ReportError
macro_rules! bail_error {
    ($self:expr, $error:expr) => {
        if let Self::ReportError { errors, .. } = $self {
            errors.push($error);
        } else {
            replace_with::replace_with_or_abort($self, |prev_self| Self::ReportError {
                errors: Vec::from([$error]),
                chain_before_first_error: Box::new(prev_self),
            });
        }
    };
}

impl ActionChain {
    // returns Some if the round terminates, with the list of people who made errors
    pub fn process_action(
        &mut self,
        player_id: PlayerId,
        action: InputPlayerAction,
        settings: &ActiveGameSettings,
        match_players: &MatchPlayers,
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
            &mut *self
        };
        match action {
            InputPlayerAction::Move(player_move) => {
                match chain {
                    // input is move, ActionChain expects a move
                    ActionChain::Moves {
                        previous_moves,
                        turn_manager,
                        move_manager,
                    } => {
                        // validate if the player is in turn
                        let previous_player =
                            previous_moves.last().map(|prev_move| &prev_move.player);
                        if !turn_manager.process(player_id, previous_player, match_players) {
                            let error = ActionError {
                                player: player_id,
                                reason: ErrorReason::MoveOutOfTurn,
                                occured: time,
                            };
                            bail_error!(self, error);
                            return None;
                        }
                        // validate if the move is correct
                        let Some(valid_move_type) = move_manager.process(
                            previous_moves.iter().map(|prev_move| &prev_move.r#type),
                            player_move,
                        ) else {
                            let error = ActionError {
                                player: player_id,
                                reason: ErrorReason::InvalidMove,
                                occured: time,
                            };
                            bail_error!(self, error);
                            return None;
                        };
                        // add the move to valid moves
                        previous_moves.push(ValidInputPlayerMove {
                            player: player_id,
                            r#type: valid_move_type,
                        });
                    }
                    // input is move, ActionChain expects a hit
                    ActionChain::Hit { .. } => {
                        let error = ActionError {
                            player: player_id,
                            reason: ErrorReason::MoveOutOfTurn,
                            occured: time,
                        };
                        bail_error!(self, error);
                    }
                    // input is move, ActionChain expects a reported error, reaction time has passed
                    ActionChain::ReportError { errors, .. } => {
                        errors.push(ActionError {
                            player: player_id,
                            reason: ErrorReason::MoveOutOfTurn,
                            occured: time,
                        });
                    }
                }
                None
            }
            InputPlayerAction::Hit(input_hit_type) => {
                match chain {
                    // input is hit, ActionChain expects a hit
                    ActionChain::Hit { players, hit_type } => {
                        // validate if the player is expected to hit
                        if !players.contains(&player_id) {
                            let error = ActionError {
                                player: player_id,
                                reason: ErrorReason::InvalidHit,
                                occured: time,
                            };
                            bail_error!(self, error);
                            return None;
                        }
                        // validate if the hit type is correct
                        if input_hit_type != *hit_type {
                            let error = ActionError {
                                player: player_id,
                                reason: ErrorReason::InvalidHitType,
                                occured: time,
                            };
                            bail_error!(self, error);
                            return None;
                        }

                        // i dont remove in the first step because the hit type may show to be wrong later,
                        // and a player can "redeem" themselves later if they hit right the next time,
                        // though they will still have made the latest error
                        players.remove(&player_id);

                        if players.is_empty() {
                            return Some(RoundTerminationType::HitPileLast(player_id));
                        }
                    }
                    // input is hit, ActionChain expects a move
                    ActionChain::Moves { .. } => {
                        let error = ActionError {
                            player: player_id,
                            reason: ErrorReason::InvalidHit,
                            occured: time,
                        };
                        bail_error!(self, error);
                    }
                    // input is move, ActionChain expects a reported error, reaction time has passed
                    ActionChain::ReportError { errors, .. } => {
                        errors.push(ActionError {
                            player: player_id,
                            reason: ErrorReason::InvalidHit,
                            occured: time,
                        });
                    }
                }
                None
            }
            // should happen during a move where that player is ruled in turn, and also terminates
            // either way. check card_piles for this
            InputPlayerAction::DeclareWin => {
                match chain {
                    // input is move, ActionChain expects a move
                    ActionChain::Moves {
                        previous_moves,
                        turn_manager,
                        ..
                    } => {
                        // validate if the player is in turn
                        let previous_player =
                            previous_moves.last().map(|prev_move| &prev_move.player);
                        if !turn_manager.process(player_id, previous_player, match_players) {
                            let error = ActionError {
                                player: player_id,
                                reason: ErrorReason::MoveOutOfTurn,
                                occured: time,
                            };
                            bail_error!(self, error);
                            return None;
                        }
                        // validate if the win declaration is correct
                        if !match_players.get_hand(&player_id).unwrap().is_empty() {
                            return Some(RoundTerminationType::FaultyWinDeclaration(player_id));
                        } else {
                            return Some(RoundTerminationType::PlayerWonMatch(player_id));
                        }
                    }
                    // maybe update state, but definitely dont need to add this to the error pile i think
                    _ => {
                        return Some(RoundTerminationType::FaultyWinDeclaration(player_id));
                    }
                }
            }
            InputPlayerAction::ReportError => {
                if let ActionChain::ReportError { errors, .. } = self {
                    return Some(RoundTerminationType::ErrorReported {
                        reporter: player_id,
                        errors: errors.clone(),
                    });
                }
                return Some(RoundTerminationType::FaultyErrorReport(player_id));
            }
        }
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
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundInfo {
    starting_player: PlayerId,
    player_actions: Vec<PlayerAction>,
    revealed_card_stacks: HashMap<PlayerId, Vec<Card>>,
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
pub struct ValidInputPlayerMove {
    pub player: PlayerId,
    pub r#type: ValidInputPlayerMoveType,
}

#[derive(PartialEq, Eq, Hash)]
pub enum ValidInputPlayerMoveType {
    CountAndLayCard(ValidCount),
    Count(ValidCount),
    LayCard,
}
impl TryFrom<InputPlayerMove> for ValidInputPlayerMoveType {
    type Error = ();

    fn try_from(value: InputPlayerMove) -> Result<Self, Self::Error> {
        Ok(match value {
            InputPlayerMove::CountAndLayCard(count) => {
                ValidInputPlayerMoveType::CountAndLayCard(ValidCount::try_from(count)?)
            }
            InputPlayerMove::Count(count) => {
                ValidInputPlayerMoveType::Count(ValidCount::try_from(count)?)
            }
            InputPlayerMove::LayCard => ValidInputPlayerMoveType::LayCard,
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
            other => Self::Time(Time::try_from(&other)?),
        })
    }
}

impl ValidInputPlayerMoveType {
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

#[derive(Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum HitType {
    // hit with right hand
    Single,
    // hit with both hands
    Double,
    // hit with right hand upside down
    UpsideDown,
}
