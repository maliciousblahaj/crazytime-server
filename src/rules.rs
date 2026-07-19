use crate::{
    card::{ClockType, Time},
    game::{
        r#match::MatchState,
        round::{ActionChain, HitType, MutableRoundState, PlayerAction},
    },
};
use rand::seq::SliceRandom;
use serde::Serialize;

pub type CriteriaFn = Box<dyn Fn(&MatchState) -> bool + Send>;

pub type RuleEffect = Box<dyn Fn(ActionChain, &MatchState) -> ActionChain + Send>;

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
    match_rules: Vec<(Criteria, Rule)>,
    // these are for selecting new active rules from
    criteria_pool: Vec<Criteria>,
    rule_pool: Vec<Rule>,
}

impl RuleManager {
    pub fn new() -> Self {
        let active_rules: Vec<(Criteria, Rule)> = vec![
            (
                Criteria {
                    description: Description::new(
                        "When the current player says the same time as the card they revealed",
                    ),
                    handler: Box::new(|match_state: &MatchState| {
                        // ok so this is absolute chaos. obviously we need to make it so criterias will only be checked after
                        // a proper valid player move, and give it in its constructor in some way, not do match_state.current_move()
                        // or maybe do such a method, but it would need so many unwraps and unnecessary panic conditions, like
                        // if there was an action since. But the state machine will guarantee that directly as a move is made,
                        // the rules will run. maybe just pass the made move as a separate argument, and don't add the move to the
                        // match state before running the actual rules, to avoid duplication. or just do match_state.get_last_move()
                        if let Some(round_state) = match_state.current_round {
                            match round_state.player_actions.last() {
                                Some(PlayerAction {
                                    player_id,
                                    time,
                                    r#type,
                                }) => todo!(),
                                None => todo!(),
                            }
                        } else {
                            false
                        }
                    }),
                },
                Rule {
                    description: Description::new(
                        "everyone should hit in the middle with one hand. the latest player loses the round",
                    ),
                    handler: Box::new(|round_state: &mut MutableRoundState, _| {
                        round_state.everyone_should_hit = Some(HitType::Single);
                    }),
                },
            ),
            (
                Criteria {
                    description: Description::new("todo"),
                    handler: Box::new(|match_state: &MatchState| {
                        match_state.current_move.card.clock == ClockType::TimeMachine
                    }),
                },
                Rule {
                    description: Description::new("todo"),
                    handler: Box::new(|round_state: &mut MutableRoundState, _| {
                        round_state.direction.toggle();
                    }),
                },
            ),
        ];
        let mut criteria_pool: Vec<Criteria> = vec![
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.clock == ClockType::Atomic
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.clock == ClockType::Watch
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.clock == ClockType::Hourglass
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.clock == ClockType::Sun
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.clock == ClockType::Chinese
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.clock == ClockType::Yellow
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.clock == ClockType::Purple
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.clock == ClockType::Purple
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.time == Time::One
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.time == Time::Two
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.time == Time::Four
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.time == Time::Five
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.time == Time::Seven
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.time == Time::Eleven
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.time == Time::Twelve
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.time.is_thirty()
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state
                        .previously_played_card
                        .as_ref()
                        .is_some_and(|previous_card| {
                            match_state.current_move.card.time == previous_card.time
                        })
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state.current_move.card.is_right_angle()
                }),
            },
            Criteria {
                description: Description::new("todo"),
                handler: Box::new(|match_state: &MatchState| {
                    match_state
                        .previously_played_card
                        .as_ref()
                        .is_some_and(|previous_card| {
                            match_state.current_move.card.clock == previous_card.clock
                        })
                }),
            },
        ];
        let mut rule_pool: Vec<Rule> = vec![
            Rule {
                description: Description::new("next player counts 1"),
                handler: Box::new(|round_state: &mut MutableRoundState, _| {
                    round_state.next_count = Time::One;
                }),
            },
            Rule {
                description: Description::new("next player is skipped"),
                handler: Box::new(
                    |round_state: &mut MutableRoundState, match_state: &MatchState| {
                        round_state.next_player_index = match round_state.direction {
                            TurnDirection::Forward => {
                                (match_state.current_player_index + 2) % match_state.n_players
                            }
                            TurnDirection::Reverse => {
                                (match_state.n_players + match_state.current_player_index - 2)
                                    % match_state.n_players
                            }
                        };
                    },
                ),
            },
            Rule {
                description: Description::new(
                    "next player counts the same time as the last player just did",
                ),
                handler: Box::new(
                    |round_state: &mut MutableRoundState, match_state: &MatchState| {
                        round_state.next_count = match_state.current_move.count;
                    },
                ),
            },
            Rule {
                description: Description::new(
                    "next player says the highest/latest time seen on the currently visible revealed cards",
                ),
                handler: Box::new(
                    |round_state: &mut MutableRoundState, match_state: &MatchState| {
                        // round_state.next_count = match_state.current_move.card.time;
                    },
                ),
            },
            Rule {
                description: Description::new("next player says the name of this rule"),
                handler: Box::new(|round_state: &mut MutableRoundState, _| {
                    round_state.should_say_the_name_of_this_rule = true;
                }),
            },
            Rule {
                description: Description::new(
                    "next player says the time that was on the card the current player played",
                ),
                handler: Box::new(
                    |round_state: &mut MutableRoundState, match_state: &MatchState| {
                        round_state.next_count = match_state.current_move.card.time;
                    },
                ),
            },
            Rule {
                description: Description::new("the current player plays again"),
                handler: Box::new(
                    |round_state: &mut MutableRoundState, match_state: &MatchState| {
                        round_state.next_player_index = match_state.current_player_index;
                    },
                ),
            },
            Rule {
                description: Description::new("the next player doesn't play a card"),
                handler: Box::new(|round_state: &mut MutableRoundState, _| {
                    round_state.should_lay_no_card = true;
                }),
            },
            Rule {
                description: Description::new(
                    "the next player says what they should've said but plus 30 minutes",
                ),
                handler: Box::new(|round_state: &mut MutableRoundState, _| {
                    round_state.next_count =
                        Time::ALL[(round_state.next_count.get_index() + 1) % 24];
                }),
            },
            Rule {
                description: Description::new(
                    "the next player says what the last player just said minus 3 hours",
                ),
                handler: Box::new(
                    |round_state: &mut MutableRoundState, match_state: &MatchState| {
                        round_state.next_count =
                            Time::ALL[(24 + match_state.current_move.count.get_index() - 6) % 24];
                    },
                ),
            },
            Rule {
                description: Description::new(
                    "the next player says what the last player just said plus 2 hours",
                ),
                handler: Box::new(
                    |round_state: &mut MutableRoundState, match_state: &MatchState| {
                        round_state.next_count =
                            Time::ALL[(match_state.current_move.count.get_index() + 4) % 24];
                    },
                ),
            },
            Rule {
                description: Description::new("the player direction is reversed"),
                handler: Box::new(|round_state: &mut MutableRoundState, _| {
                    round_state.direction.toggle();
                }),
            },
            Rule {
                description: Description::new("the count interval should now be 2 hours at a time"),
                handler: Box::new(|round_state: &mut MutableRoundState, _| {
                    round_state.count_interval_index = 4;
                }),
            },
            Rule {
                description: Description::new(
                    "the count interval should now be 30 minutes at a time",
                ),
                handler: Box::new(|round_state: &mut MutableRoundState, _| {
                    round_state.count_interval_index = 1;
                }),
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
                handler: Box::new(|round_state: &mut MutableRoundState, _| {
                    round_state.everyone_should_hit = Some(HitType::Double);
                }),
            },
            Rule {
                description: Description::new(
                    "everyone should hit in the middle with the palm up. the latest player loses the round",
                ),
                handler: Box::new(|round_state: &mut MutableRoundState, _| {
                    round_state.everyone_should_hit = Some(HitType::UpsideDown);
                }),
            },
            Rule {
                description: Description::new(
                    "the winner of the previous round should do the next move",
                ),
                handler: Box::new(|round_state: &mut MutableRoundState, _| {}),
            },
            Rule {
                description: Description::new(
                    "the player with the highest/latest time on the top of their revealed card stack should do the next move",
                ),
                handler: Box::new(|round_state: &mut MutableRoundState, _| {}),
            },
            Rule {
                description: Description::new(
                    "the next player should say anything but their supposed time",
                ),
                handler: Box::new(|round_state: &mut MutableRoundState, _| {
                    round_state.should_say_anything_but_correct_count = true;
                }),
            },
            Rule {
                description: Description::new(
                    "the players left and right of the current player must hit in the middle. the latest player loses the round",
                ),
                handler: Box::new(|round_state: &mut MutableRoundState, _| {}),
            },
            Rule {
                description: Description::new(
                    "from now on all players should say anything but their supposed time",
                ),
                handler: Box::new(|round_state: &mut MutableRoundState, _| {
                    round_state.everyone_should_say_anything_but_correct_count = true;
                }),
            },
            // Rule {
            //     description: Description::new(
            //         "the players left and right of the current player must hit in the middle",
            //     ),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {}),
            // },
            // Rule {
            //     description: Description::new(
            //         "all players with even numbers on their card stack should hit in the middle",
            //     ),
            //     handler: Box::new(|round_state: &mut MutableRoundState, _| {}),
            // },
        ];

        let mut rng = rand::rng();
        criteria_pool.shuffle(&mut rng);
        rule_pool.shuffle(&mut rng);

        Self {
            active_rules,
            criteria_pool,
            rule_pool,
        }
    }

    /// run all rules
    pub fn run_rules(&self, match_state: &MatchState, round_state: &mut MutableRoundState) {
        let mut rule_to_run = None;
        for (criteria, rule) in self
            .active_rules
            .iter()
            .map(|(criteria, rule)| (&criteria.handler, &rule.handler))
        {
            if criteria(match_state) {
                if rule_to_run.is_some() {
                    // double rule, no rules apply
                    return;
                }
                rule_to_run = Some(rule);
            }
        }
        if let Some(rule) = rule_to_run {
            rule(round_state, match_state);
        }
    }

    /// returns Err if there are no more rules
    pub fn add_rule(&mut self) -> Result<(), ()> {
        self.active_rules.push((
            self.criteria_pool.pop().ok_or(())?,
            self.rule_pool.pop().ok_or(())?,
        ));
        Ok(())
    }
}
