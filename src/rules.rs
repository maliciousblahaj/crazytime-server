use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use crate::{
    card::{Card, ClockType, Time},
    game::{
        GameContext,
        round::{
            ActionChain, ActionChainMoves, Count, HitType, TimeInterval, ValidCount,
            ValidInputPlayerMoveType,
        },
    },
};
use rand::seq::SliceRandom;
use serde::Serialize;

pub type CriteriaFn = Box<dyn Fn(&GameContext) -> bool + Send>;

pub struct Criteria {
    description: Description,
    handler: CriteriaFn,
}

pub type RuleEffectFn = Arc<dyn Fn(ActionChainMoves, &GameContext) -> ActionChain + Send + Sync>;

#[derive(Clone)]
pub struct RuleEffect {
    pub handler: RuleEffectFn,
    pub duration: RuleEffectDuration,
}

impl RuleEffect {
    pub fn new(handler: RuleEffectFn, duration: RuleEffectDuration) -> Self {
        Self { handler, duration }
    }
}

#[derive(Clone)]
pub enum RuleEffectDuration {
    RestOfRound,
    NTimes(usize),
}

impl RuleEffectDuration {
    pub const ONCE: Self = Self::NTimes(1);

    /// returns true if it has reached zero
    pub fn decrease(&mut self) -> bool {
        if let RuleEffectDuration::NTimes(n) = self {
            *n -= 1;
            if *n == 0 {
                return true;
            }
        }
        false
    }
}

pub struct Rule {
    description: Description,
    effect: RuleEffect,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInfo {
    id: usize,
    rule: Description,
    criteria: Description,
}

// potential future support of multiple languages
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Description {
    english: String,
}

impl Description {
    pub fn new(english: impl AsRef<str>) -> Self {
        Self {
            english: english.as_ref().to_string(),
        }
    }
}

pub struct RuleManager {
    game_rules: BTreeMap<usize, (Criteria, Rule)>,
    next_game_rule_id: usize,
    // these are for selecting new active rules from
    criteria_pool: Vec<Criteria>,
    rule_pool: Vec<Rule>,
}

impl RuleManager {
    pub fn new() -> Self {
        let mut game_rules: BTreeMap<usize, (Criteria, Rule)> = BTreeMap::new();
        game_rules.insert(0,
            (
                Criteria {
                    description: Description::new(
                        "When the current player says the same time as the card they revealed",
                    ),
                    handler: Box::new(|game_ctx: &GameContext| {
                        if let Some(count) = game_ctx.get_just_said_count()
                            && let Some(Card { time, .. }) = game_ctx.get_just_revealed_card()
                            && TryInto::<Time>::try_into(&count).is_ok_and(|count| count == time)
                        {
                            true
                        } else {
                            false
                        }
                    }),
                },
                Rule {
                    description: Description::new(
                        "everyone should hit in the middle with one hand. the latest player loses the round",
                    ),
                    effect: RuleEffect::new(
                        Arc::new(|_, game_ctx: &GameContext| ActionChain::Hit {
                            players: game_ctx.players.iter().copied().collect(),
                            hit_type: HitType::Single,
                        }),
                        RuleEffectDuration::ONCE,
                    ),
                },
            ));
        game_rules.insert(
            1,
            (
                Criteria {
                    description: Description::new("When the revealed card is of type TimeMachine"),
                    handler: Box::new(|game_ctx: &GameContext| {
                        if let Some(Card { clock, .. }) = game_ctx.get_just_revealed_card()
                            && clock == ClockType::TimeMachine
                        {
                            true
                        } else {
                            false
                        }
                    }),
                },
                Rule {
                    description: Description::new("from now on the count direction should reverse"),
                    effect: RuleEffect::new(
                        Arc::new(|action_chain_moves: ActionChainMoves, _| {
                            let mut move_manager = action_chain_moves.move_manager;
                            move_manager.count_interval.toggle_direction();
                            ActionChain::Moves {
                                previous_moves: action_chain_moves.previous_moves,
                                turn_manager: action_chain_moves.turn_manager,
                                move_manager,
                            }
                        }),
                        RuleEffectDuration::ONCE,
                    ),
                },
            ),
        );
        let mut criteria_pool: Vec<Criteria> = vec![
            Criteria {
                description: Description::new("When the revealed card is of type Atomic"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { clock, .. }) = game_ctx.get_just_revealed_card()
                        && clock == ClockType::Atomic
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed card is of type Watch"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { clock, .. }) = game_ctx.get_just_revealed_card()
                        && clock == ClockType::Watch
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed card is of type Hourglass"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { clock, .. }) = game_ctx.get_just_revealed_card()
                        && clock == ClockType::Hourglass
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed card is of type Sun"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { clock, .. }) = game_ctx.get_just_revealed_card()
                        && clock == ClockType::Sun
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed card is of type Chinese"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { clock, .. }) = game_ctx.get_just_revealed_card()
                        && clock == ClockType::Chinese
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed card is of type Yellow"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { clock, .. }) = game_ctx.get_just_revealed_card()
                        && clock == ClockType::Yellow
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed card is of type Purple"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { clock, .. }) = game_ctx.get_just_revealed_card()
                        && clock == ClockType::Purple
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed cards shows 01:00"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { time, .. }) = game_ctx.get_just_revealed_card()
                        && time == Time::One
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed cards shows 02:00"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { time, .. }) = game_ctx.get_just_revealed_card()
                        && time == Time::Two
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed cards shows 04:00"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { time, .. }) = game_ctx.get_just_revealed_card()
                        && time == Time::Four
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed cards shows 05:00"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { time, .. }) = game_ctx.get_just_revealed_card()
                        && time == Time::Five
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed cards shows 07:00"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { time, .. }) = game_ctx.get_just_revealed_card()
                        && time == Time::Seven
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed cards shows 11:00"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { time, .. }) = game_ctx.get_just_revealed_card()
                        && time == Time::Eleven
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed cards shows 12:00"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { time, .. }) = game_ctx.get_just_revealed_card()
                        && time == Time::Twelve
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new("When the revealed cards shows xx:30"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { time, .. }) = game_ctx.get_just_revealed_card()
                        && time.is_thirty()
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new(
                    "When the revealed card has the same time as the previously revealed card",
                ),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(last_cards) = game_ctx.get_n_previous_cards(2)
                        && last_cards[0].time == last_cards[1].time
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new(
                    "When the revealed card's clock's hands are in a right angle",
                ),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(card) = game_ctx.get_just_revealed_card()
                        && card.is_right_angle()
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
            Criteria {
                description: Description::new(
                    "When the revealed card has the same clock as the previously revealed card",
                ),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(last_cards) = game_ctx.get_n_previous_cards(2)
                        && last_cards[0].clock == last_cards[1].clock
                    {
                        true
                    } else {
                        false
                    }
                }),
            },
        ];
        let mut rule_pool: Vec<Rule> = vec![
            // todo make this the vec macro again, just doing this for better lsp support
            Rule {
                description: Description::new("next player should count 01:00"),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             turn_manager,
                             mut move_manager,
                         },
                         _| {
                            move_manager.set_next_move(ValidInputPlayerMoveType::CountAndLayCard(
                                ValidCount::Time(Time::One),
                            ));
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new("next player is skipped in turn"),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             mut turn_manager,
                             move_manager,
                         },
                         game_ctx| {
                            let current_player = game_ctx.previous_moves.last().unwrap().0;
                            let current_index = game_ctx
                                .players
                                .iter()
                                .position(|plr| *plr == current_player)
                                .unwrap();
                            let new_index = (current_index as isize + 2 * turn_manager.direction.0)
                                .rem_euclid(game_ctx.players.len() as isize);

                            turn_manager.set_next_player(game_ctx.players[new_index as usize]);

                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "next player should count the same time as the current player just did",
                ),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             turn_manager,
                             mut move_manager,
                         },
                         game_ctx| {
                            if let Some(last_count) = game_ctx.get_just_said_count() {
                                move_manager.set_next_move(
                                    ValidInputPlayerMoveType::CountAndLayCard(
                                        ValidCount::try_from(last_count).unwrap(),
                                    ),
                                );
                            } else {
                                move_manager.set_next_move(ValidInputPlayerMoveType::LayCard);
                            }
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "next player should count the highest/latest time seen on the currently visible revealed cards",
                ),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             turn_manager,
                             mut move_manager,
                         },
                         game_ctx| {
                            let highest_time = game_ctx
                                .revealed_card_stacks
                                .iter()
                                .flat_map(|(_player, stack)| stack.last())
                                .fold(None, |acc, candidate| {
                                    if acc.is_some_and(|best_time| best_time >= candidate.time) {
                                        acc
                                    } else {
                                        Some(candidate.time)
                                    }
                                })
                                .unwrap();

                            move_manager.set_next_move(ValidInputPlayerMoveType::CountAndLayCard(
                                ValidCount::Time(highest_time),
                            ));
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new("next player should say the name of this rule"),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             turn_manager,
                             mut move_manager,
                         },
                         _| {
                            move_manager.set_next_move(ValidInputPlayerMoveType::CountAndLayCard(
                                ValidCount::NameOfThisRule,
                            ));
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "next player should count the same time that was on the latest revealed card",
                ),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             turn_manager,
                             mut move_manager,
                         },
                         game_ctx| {
                            let Some(card) = game_ctx.get_previous_cards().last() else {
                                return ActionChain::Moves {
                                    previous_moves,
                                    turn_manager,
                                    move_manager,
                                };
                            };
                            move_manager.set_next_move(ValidInputPlayerMoveType::CountAndLayCard(
                                ValidCount::Time(card.time),
                            ));
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new("the current player should play again"),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             mut turn_manager,
                             move_manager,
                         },
                         game_ctx| {
                            let current_player = game_ctx.previous_moves.last().unwrap().0;

                            turn_manager.set_next_player(current_player);

                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new("next player should not reveal a card"),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             turn_manager,
                             mut move_manager,
                         },
                         _| {
                            let expected_time = move_manager.get_next_expected_time(
                                previous_moves
                                    .iter()
                                    .rev()
                                    .map(|prev_move| &prev_move.r#type),
                            );
                            move_manager.set_next_move(ValidInputPlayerMoveType::Count(
                                ValidCount::Time(expected_time),
                            ));
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "next player should say what they were supposed to say plus 30 minutes",
                ),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             turn_manager,
                             mut move_manager,
                         },
                         _| {
                            let expected_time = move_manager.get_next_expected_time(
                                previous_moves
                                    .iter()
                                    .rev()
                                    .map(|prev_move| &prev_move.r#type),
                            );
                            move_manager.set_next_move(ValidInputPlayerMoveType::CountAndLayCard(
                                ValidCount::Time(expected_time.plus(&TimeInterval::THIRTYMINUTES)),
                            ));
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "next player should say what the last player said minus 3 hours",
                ),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             turn_manager,
                             mut move_manager,
                         },
                         game_ctx| {
                            let Some(last_time) = game_ctx
                                .get_previous_counts()
                                .find(|said| *said != Count::NameOfThisRule)
                                .map(|count| Time::try_from(&count).unwrap())
                            else {
                                return ActionChain::Moves {
                                    previous_moves,
                                    turn_manager,
                                    move_manager,
                                };
                            };

                            move_manager.set_next_move(ValidInputPlayerMoveType::CountAndLayCard(
                                ValidCount::Time(last_time.plus(&TimeInterval::MINUSTHREEHOURS)),
                            ));
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "next player should say what the last player said plus 2 hours",
                ),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             turn_manager,
                             mut move_manager,
                         },
                         game_ctx| {
                            let Some(last_time) = game_ctx
                                .get_previous_counts()
                                .find(|said| *said != Count::NameOfThisRule)
                                .map(|count| Time::try_from(&count).unwrap())
                            else {
                                return ActionChain::Moves {
                                    previous_moves,
                                    turn_manager,
                                    move_manager,
                                };
                            };
                            move_manager.set_next_move(ValidInputPlayerMoveType::CountAndLayCard(
                                ValidCount::Time(last_time.plus(&TimeInterval::TWOHOURS)),
                            ));
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new("from now on the player direction is reversed"),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             mut turn_manager,
                             move_manager,
                         },
                         _| {
                            turn_manager.direction.toggle_direction();
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "from now on the count interval is in steps of 2 hours at a time",
                ),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             turn_manager,
                             mut move_manager,
                         },
                         _| {
                            move_manager
                                .count_interval
                                .set_step_size(TimeInterval::TWOHOURS);
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "from now on the count interval is in steps of 30 minutes at a time",
                ),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             turn_manager,
                             mut move_manager,
                         },
                         _| {
                            move_manager
                                .count_interval
                                .set_step_size(TimeInterval::THIRTYMINUTES);
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            // Rule {
            //     description: Description::new(
            //         "from now on, everyone should count in another language or dialect than their own",
            //     ),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {}),
            // },
            Rule {
                description: Description::new(
                    "everyone should hit in the middle with both hands. the latest player loses the round",
                ),
                effect: RuleEffect::new(
                    Arc::new(|_, game_ctx| ActionChain::Hit {
                        players: game_ctx.players.iter().copied().collect(),
                        hit_type: HitType::Double,
                    }),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "everyone should hit in the middle with the palm up. the latest player loses the round",
                ),
                effect: RuleEffect::new(
                    Arc::new(|_, game_ctx| ActionChain::Hit {
                        players: game_ctx.players.iter().copied().collect(),
                        hit_type: HitType::UpsideDown,
                    }),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "the winner of the previous match should do the next move",
                ),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             mut turn_manager,
                             move_manager,
                         },
                         game_ctx| {
                            let Some(winner) = game_ctx.previous_match_winner else {
                                return ActionChain::Moves {
                                    previous_moves,
                                    turn_manager,
                                    move_manager,
                                };
                            };
                            turn_manager.set_next_player(winner);
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "the player with the highest/latest time seen on top of their currently visible revealed card stack should do the next move",
                ),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             mut turn_manager,
                             move_manager,
                         },
                         game_ctx| {
                            let Some((highest_time_player, _)) = game_ctx
                                .revealed_card_stacks
                                .iter()
                                .flat_map(|(player, stack)| stack.last().map(|card| (player, card)))
                                .fold(None, |acc, candidate| {
                                    if acc.is_some_and(|(_player, best_time)| {
                                        best_time >= candidate.1.time
                                    }) {
                                        acc
                                    } else {
                                        Some((candidate.0, candidate.1.time))
                                    }
                                })
                            else {
                                return ActionChain::Moves {
                                    previous_moves,
                                    turn_manager,
                                    move_manager,
                                };
                            };
                            turn_manager.set_next_player(*highest_time_player);
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "next player should say any valid time except the time they're supposed to count",
                ),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             turn_manager,
                             mut move_manager,
                         },
                         _| {
                            let expected_time = move_manager.get_next_expected_time(
                                previous_moves
                                    .iter()
                                    .rev()
                                    .map(|prev_move| &prev_move.r#type),
                            );
                            move_manager.set_next_moves(
                                Time::ALL
                                    .into_iter()
                                    .filter(|time| *time != expected_time)
                                    .map(|time| {
                                        ValidInputPlayerMoveType::CountAndLayCard(ValidCount::Time(
                                            time,
                                        ))
                                    })
                                    .collect(),
                            );
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "the players left and right of the current player should hit in the middle. the latest player loses the round",
                ),
                effect: RuleEffect::new(
                    Arc::new(|_, game_ctx| {
                        let current_player = game_ctx.previous_moves.last().unwrap().0;
                        let index = game_ctx
                            .players
                            .iter()
                            .position(|player| *player == current_player)
                            .unwrap();
                        let (player_1, player_2) = (
                            game_ctx.players[(index as isize - 1)
                                .rem_euclid(game_ctx.players.len() as isize)
                                as usize],
                            game_ctx.players[(index as isize + 1)
                                .rem_euclid(game_ctx.players.len() as isize)
                                as usize],
                        );

                        ActionChain::Hit {
                            players: HashSet::from([player_1, player_2]),
                            hit_type: HitType::Single,
                        }
                    }),
                    RuleEffectDuration::ONCE,
                ),
            },
            Rule {
                description: Description::new(
                    "from now on all players should say any valid time except the time they're supposed to count",
                ),
                effect: RuleEffect::new(
                    Arc::new(
                        |ActionChainMoves {
                             previous_moves,
                             turn_manager,
                             mut move_manager,
                         },
                         _| {
                            let expected_time = move_manager.get_next_expected_time(
                                previous_moves
                                    .iter()
                                    .rev()
                                    .map(|prev_move| &prev_move.r#type),
                            );
                            move_manager.set_next_moves(
                                Time::ALL
                                    .into_iter()
                                    .filter(|time| *time != expected_time)
                                    .map(|time| {
                                        ValidInputPlayerMoveType::CountAndLayCard(ValidCount::Time(
                                            time,
                                        ))
                                    })
                                    .collect(),
                            );
                            ActionChain::Moves {
                                previous_moves,
                                turn_manager,
                                move_manager,
                            }
                        },
                    ),
                    RuleEffectDuration::RestOfRound,
                ),
            },
            // Rule {
            //     description: Description::new(
            //         "all players with even time on the top of their card stack should hit in the middle. the latest player loses the round",
            //     ),
            //     effect: RuleEffect::new(
            //         Arc::new(|_, game_ctx| {
            // //             let current_player = game_ctx.previous_moves.last().unwrap().0;
            // //             let index = game_ctx
            // //                 .players
            // //                 .iter()
            // //                 .position(|player| *player == current_player)
            // //                 .unwrap();
            // //             let (player_1, player_2) = (
            // //                 game_ctx.players[(index as isize - 1)
            // //                     .rem_euclid(game_ctx.players.len() as isize)
            // //                     as usize],
            // //                 game_ctx.players[(index as isize + 1)
            // //                     .rem_euclid(game_ctx.players.len() as isize)
            // //                     as usize],
            // //             );

            // //             ActionChain::Hit {
            // //                 players: HashSet::from([player_1, player_2]),
            // //                 hit_type: HitType::Single,
            // //             }
            //         }),
            //         RuleEffectDuration::ONCE,
            //     ),
            // },
        ];

        let mut rng = rand::rng();
        criteria_pool.shuffle(&mut rng);
        rule_pool.shuffle(&mut rng);

        Self {
            game_rules,
            next_game_rule_id: 2,
            criteria_pool,
            rule_pool,
        }
    }

    pub fn get_new_active_effect(&self, game_ctx: &GameContext) -> Option<RuleEffect> {
        let mut new_rule_effect = None;
        for (criteria, rule_effect) in self
            .game_rules
            .iter()
            .map(|(_id, (criteria, rule))| (&criteria.handler, &rule.effect))
        {
            if criteria(game_ctx) {
                if new_rule_effect.is_some() {
                    // double rule, no rules apply
                    return None;
                }
                new_rule_effect = Some(rule_effect.clone());
            }
        }

        new_rule_effect
    }

    /// returns Err if there are no more rules
    pub fn add_rule(&mut self) -> Result<RuleInfo, ()> {
        let (criteria, rule) = (
            self.criteria_pool.pop().ok_or(())?,
            self.rule_pool.pop().ok_or(())?,
        );
        let (criteria_desc, rule_desc) = (criteria.description.clone(), rule.description.clone());
        let id = self.next_game_rule_id;
        self.next_game_rule_id += 1;
        self.game_rules.insert(id, (criteria, rule));
        Ok(RuleInfo {
            id,
            rule: rule_desc,
            criteria: criteria_desc,
        })
    }

    /// returns true if the rule was successfully removed
    ///
    /// will reinsert the criteria and rule to the pool
    pub fn remove_rule(&mut self, id: &usize) -> bool {
        if let Some((criteria, rule)) = self.game_rules.remove(id) {
            self.criteria_pool.insert(0, criteria);
            self.rule_pool.insert(0, rule);
            true
        } else {
            false
        }
    }

    pub fn active_rules_info(&self) -> Vec<RuleInfo> {
        self.game_rules
            .iter()
            .map(|(id, (crit, rule))| RuleInfo {
                id: *id,
                criteria: crit.description.clone(),
                rule: rule.description.clone(),
            })
            .collect()
    }
}
