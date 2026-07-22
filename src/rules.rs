use crate::{
    card::{Card, ClockType, Time},
    game::{
        GameContext,
        round::{ActionChain, ActionChainMoves, HitType},
    },
};
use rand::seq::SliceRandom;
use serde::Serialize;

pub type CriteriaFn = Box<dyn Fn(&GameContext) -> bool + Send>;

pub type RuleEffect = Box<dyn Fn(ActionChainMoves, &GameContext) -> ActionChain + Send>;

pub struct Criteria {
    description: Description,
    handler: CriteriaFn,
}
pub struct Rule {
    description: Description,
    handler: RuleEffect,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInfo {
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
    game_rules: Vec<(Criteria, Rule)>,
    // these are for selecting new active rules from
    criteria_pool: Vec<Criteria>,
    rule_pool: Vec<Rule>,
}

impl RuleManager {
    pub fn new() -> Self {
        let game_rules: Vec<(Criteria, Rule)> = vec![
            (
                Criteria {
                    description: Description::new(
                        "When the current player says the same time as the card they revealed",
                    ),
                    handler: Box::new(|game_ctx: &GameContext| {
                        if let Some(count) = game_ctx.get_just_said_count()
                            && let Some(Card { time, .. }) = game_ctx.get_just_revealed_card()
                            && TryInto::<Time>::try_into(count).is_ok_and(|count| count == *time)
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
                    handler: Box::new(|_, game_ctx: &GameContext| ActionChain::Hit {
                        players: game_ctx.players.iter().copied().collect(),
                        hit_type: HitType::Single,
                    }),
                },
            ),
            (
                Criteria {
                    description: Description::new("When the revealed card is of type TimeMachine"),
                    handler: Box::new(|game_ctx: &GameContext| {
                        if let Some(Card { clock, .. }) = game_ctx.get_just_revealed_card()
                            && *clock == ClockType::TimeMachine
                        {
                            true
                        } else {
                            false
                        }
                    }),
                },
                Rule {
                    description: Description::new("from now on the count direction should reverse"),
                    handler: Box::new(|action_chain_moves: ActionChainMoves, _| {
                        let mut move_manager = action_chain_moves.move_manager;
                        move_manager.count_interval.toggle_direction();
                        ActionChain::Moves {
                            previous_moves: action_chain_moves.previous_moves,
                            turn_manager: action_chain_moves.turn_manager,
                            move_manager,
                        }
                    }),
                },
            ),
        ];
        let mut criteria_pool: Vec<Criteria> = vec![
            Criteria {
                description: Description::new("When the revealed card is of type Atomic"),
                handler: Box::new(|game_ctx: &GameContext| {
                    if let Some(Card { clock, .. }) = game_ctx.get_just_revealed_card()
                        && *clock == ClockType::Atomic
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
                        && *clock == ClockType::Watch
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
                        && *clock == ClockType::Hourglass
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
                        && *clock == ClockType::Sun
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
                        && *clock == ClockType::Chinese
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
                        && *clock == ClockType::Yellow
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
                        && *clock == ClockType::Purple
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
                        && *time == Time::One
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
                        && *time == Time::Two
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
                        && *time == Time::Four
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
                        && *time == Time::Five
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
                        && *time == Time::Seven
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
                        && *time == Time::Eleven
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
                        && *time == Time::Twelve
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
                    todo!()
                    // match_state
                    //     .previously_played_card
                    //     .as_ref()
                    //     .is_some_and(|previous_card| {
                    //         match_state.current_move.card.time == previous_card.time
                    //     })
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
                    todo!()
                    // match_state
                    //     .previously_played_card
                    //     .as_ref()
                    //     .is_some_and(|previous_card| {
                    //         match_state.current_move.card.clock == previous_card.clock
                    //     })
                }),
            },
        ];
        let mut rule_pool: Vec<Rule> = vec![
            // Rule {
            //     description: Description::new("next player counts 1"),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {
            //         round_state.next_count = Time::One;
            //     }),
            // },
            // Rule {
            //     description: Description::new("next player is skipped"),
            //     handler: Box::new(
            //         |round_state: &mut MutableRoundState, match_state: &MatchState| {
            //             round_state.next_player_index = match round_state.direction {
            //                 TurnDirection::Forward => {
            //                     (match_state.current_player_index + 2) % match_state.n_players
            //                 }
            //                 TurnDirection::Reverse => {
            //                     (match_state.n_players + match_state.current_player_index - 2)
            //                         % match_state.n_players
            //                 }
            //             };
            //         },
            //     ),
            // },
            // Rule {
            //     description: Description::new(
            //         "next player counts the same time as the last player just did",
            //     ),
            //     handler: Box::new(
            //         |round_state: &mut MutableRoundState, match_state: &MatchState| {
            //             round_state.next_count = match_state.current_move.count;
            //         },
            //     ),
            // },
            // Rule {
            //     description: Description::new(
            //         "next player says the highest/latest time seen on the currently visible revealed cards",
            //     ),
            //     handler: Box::new(
            //         |round_state: &mut MutableRoundState, match_state: &MatchState| {
            //             // round_state.next_count = match_state.current_move.card.time;
            //         },
            //     ),
            // },
            // Rule {
            //     description: Description::new("next player says the name of this rule"),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {
            //         round_state.should_say_the_name_of_this_rule = true;
            //     }),
            // },
            // Rule {
            //     description: Description::new(
            //         "next player says the time that was on the card the current player played",
            //     ),
            //     handler: Box::new(
            //         |round_state: &mut MutableRoundState, match_state: &MatchState| {
            //             round_state.next_count = match_state.current_move.card.time;
            //         },
            //     ),
            // },
            // Rule {
            //     description: Description::new("the current player plays again"),
            //     handler: Box::new(
            //         |round_state: &mut MutableRoundState, match_state: &MatchState| {
            //             round_state.next_player_index = match_state.current_player_index;
            //         },
            //     ),
            // },
            // Rule {
            //     description: Description::new("the next player doesn't play a card"),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {
            //         round_state.should_lay_no_card = true;
            //     }),
            // },
            // Rule {
            //     description: Description::new(
            //         "the next player says what they should've said but plus 30 minutes",
            //     ),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {
            //         round_state.next_count =
            //             Time::ALL[(round_state.next_count.get_index() + 1) % 24];
            //     }),
            // },
            // Rule {
            //     description: Description::new(
            //         "the next player says what the last player just said minus 3 hours",
            //     ),
            //     handler: Box::new(
            //         |round_state: &mut MutableRoundState, match_state: &MatchState| {
            //             round_state.next_count =
            //                 Time::ALL[(24 + match_state.current_move.count.get_index() - 6) % 24];
            //         },
            //     ),
            // },
            // Rule {
            //     description: Description::new(
            //         "the next player says what the last player just said plus 2 hours",
            //     ),
            //     handler: Box::new(
            //         |round_state: &mut MutableRoundState, match_state: &MatchState| {
            //             round_state.next_count =
            //                 Time::ALL[(match_state.current_move.count.get_index() + 4) % 24];
            //         },
            //     ),
            // },
            // Rule {
            //     description: Description::new("the player direction is reversed"),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {
            //         round_state.direction.toggle();
            //     }),
            // },
            // Rule {
            //     description: Description::new("the count interval should now be 2 hours at a time"),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {
            //         round_state.count_interval_index = 4;
            //     }),
            // },
            // Rule {
            //     description: Description::new(
            //         "the count interval should now be 30 minutes at a time",
            //     ),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {
            //         round_state.count_interval_index = 1;
            //     }),
            // },
            // // Rule {
            // //     description: Description::new(
            // //         "from now on, everyone should count in another language or dialect than their own",
            // //     ),
            // //     handler: Box::new(|round_state: &mut MutableRoundState, _| {}),
            // // },
            // Rule {
            //     description: Description::new(
            //         "everyone should hit in the middle with both hands. the latest player loses the round",
            //     ),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {
            //         round_state.everyone_should_hit = Some(HitType::Double);
            //     }),
            // },
            // Rule {
            //     description: Description::new(
            //         "everyone should hit in the middle with the palm up. the latest player loses the round",
            //     ),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {
            //         round_state.everyone_should_hit = Some(HitType::UpsideDown);
            //     }),
            // },
            // Rule {
            //     description: Description::new(
            //         "the winner of the previous round should do the next move",
            //     ),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {}),
            // },
            // Rule {
            //     description: Description::new(
            //         "the player with the highest/latest time on the top of their revealed card stack should do the next move",
            //     ),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {}),
            // },
            // Rule {
            //     description: Description::new(
            //         "the next player should say anything but their supposed time",
            //     ),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {
            //         round_state.should_say_anything_but_correct_count = true;
            //     }),
            // },
            // Rule {
            //     description: Description::new(
            //         "the players left and right of the current player must hit in the middle. the latest player loses the round",
            //     ),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {}),
            // },
            // Rule {
            //     description: Description::new(
            //         "from now on all players should say anything but their supposed time",
            //     ),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {
            //         round_state.everyone_should_say_anything_but_correct_count = true;
            //     }),
            // },
            // // Rule {
            // //     description: Description::new(
            // //         "the players left and right of the current player must hit in the middle",
            // //     ),
            // //     handler: Box::new(|round_state: &mut MutableRoundState, _| {}),
            // // },
            // // Rule {
            // //     description: Description::new(
            // //         "all players with even numbers on their card stack should hit in the middle",
            // //     ),
            // //     handler: Box::new(|round_state: &mut MutableRoundState, _| {}),
            // // },
        ];

        let mut rng = rand::rng();
        criteria_pool.shuffle(&mut rng);
        rule_pool.shuffle(&mut rng);

        Self {
            game_rules,
            criteria_pool,
            rule_pool,
        }
    }

    // /// run all rules
    // pub fn run_rules(&self, match_state: &MatchState, round_state: &mut MutableRoundState) {
    //     let mut rule_to_run = None;
    //     for (criteria, rule) in self
    //         .active_rules
    //         .iter()
    //         .map(|(criteria, rule)| (&criteria.handler, &rule.handler))
    //     {
    //         if criteria(match_state) {
    //             if rule_to_run.is_some() {
    //                 // double rule, no rules apply
    //                 return;
    //             }
    //             rule_to_run = Some(rule);
    //         }
    //     }
    //     if let Some(rule) = rule_to_run {
    //         rule(round_state, match_state);
    //     }
    // }

    /// returns Err if there are no more rules
    pub fn add_rule(&mut self) -> Result<RuleInfo, ()> {
        let (rule, criteria) = (
            self.criteria_pool.pop().ok_or(())?,
            self.rule_pool.pop().ok_or(())?,
        );
        let (rule_desc, criteria_desc) = (rule.description.clone(), criteria.description.clone());
        self.game_rules.push((rule, criteria));
        Ok(RuleInfo {
            rule: rule_desc,
            criteria: criteria_desc,
        })
    }

    pub fn active_rules(&self) -> Vec<RuleInfo> {
        self.game_rules
            .iter()
            .map(|(crit, rule)| RuleInfo {
                criteria: crit.description.clone(),
                rule: rule.description.clone(),
            })
            .collect()
    }
}
