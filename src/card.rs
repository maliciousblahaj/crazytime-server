use std::collections::VecDeque;

use rand::seq::SliceRandom;
use serde::Serialize;

use crate::game::round::{Count, TimeInterval};

pub struct CardPool(VecDeque<Card>);

impl CardPool {
    /// with fixed card pool
    pub fn new() -> Self {
        let mut card_pool = [
            // sun
            Card {
                clock: ClockType::Sun,
                time: Time::One,
            },
            Card {
                clock: ClockType::Sun,
                time: Time::Three,
            },
            Card {
                clock: ClockType::Sun,
                time: Time::Four,
            },
            Card {
                clock: ClockType::Sun,
                time: Time::Five,
            },
            Card {
                clock: ClockType::Sun,
                time: Time::Six,
            },
            Card {
                clock: ClockType::Sun,
                time: Time::SevenThirty,
            },
            Card {
                clock: ClockType::Sun,
                time: Time::Eleven,
            },
            Card {
                clock: ClockType::Sun,
                time: Time::Twelve,
            },
            // hourglass
            Card {
                clock: ClockType::Hourglass,
                time: Time::One,
            },
            Card {
                clock: ClockType::Hourglass,
                time: Time::Two,
            },
            Card {
                clock: ClockType::Hourglass,
                time: Time::Three,
            },
            Card {
                clock: ClockType::Hourglass,
                time: Time::Four,
            },
            Card {
                clock: ClockType::Hourglass,
                time: Time::Five,
            },
            Card {
                clock: ClockType::Hourglass,
                time: Time::Six,
            },
            Card {
                clock: ClockType::Hourglass,
                time: Time::Ten,
            },
            Card {
                clock: ClockType::Hourglass,
                time: Time::Eleven,
            },
            // chinese
            Card {
                clock: ClockType::Chinese,
                time: Time::One,
            },
            Card {
                clock: ClockType::Chinese,
                time: Time::Two,
            },
            Card {
                clock: ClockType::Chinese,
                time: Time::Four,
            },
            Card {
                clock: ClockType::Chinese,
                time: Time::Five,
            },
            Card {
                clock: ClockType::Chinese,
                time: Time::Six,
            },
            Card {
                clock: ClockType::Chinese,
                time: Time::Seven,
            },
            Card {
                clock: ClockType::Chinese,
                time: Time::EightThirty,
            },
            Card {
                clock: ClockType::Chinese,
                time: Time::Twelve,
            },
            // yellow
            Card {
                clock: ClockType::Yellow,
                time: Time::One,
            },
            Card {
                clock: ClockType::Yellow,
                time: Time::Two,
            },
            Card {
                clock: ClockType::Yellow,
                time: Time::Three,
            },
            Card {
                clock: ClockType::Yellow,
                time: Time::Five,
            },
            Card {
                clock: ClockType::Yellow,
                time: Time::Nine,
            },
            Card {
                clock: ClockType::Yellow,
                time: Time::Ten,
            },
            Card {
                clock: ClockType::Yellow,
                time: Time::Eleven,
            },
            // purple
            Card {
                clock: ClockType::Purple,
                time: Time::Two,
            },
            Card {
                clock: ClockType::Purple,
                time: Time::Six,
            },
            Card {
                clock: ClockType::Purple,
                time: Time::Seven,
            },
            Card {
                clock: ClockType::Purple,
                time: Time::Eight,
            },
            Card {
                clock: ClockType::Purple,
                time: Time::Nine,
            },
            Card {
                clock: ClockType::Purple,
                time: Time::TenThirty,
            },
            Card {
                clock: ClockType::Purple,
                time: Time::Twelve,
            },
            // watch
            Card {
                clock: ClockType::Watch,
                time: Time::One,
            },
            Card {
                clock: ClockType::Watch,
                time: Time::Two,
            },
            Card {
                clock: ClockType::Watch,
                time: Time::Seven,
            },
            Card {
                clock: ClockType::Watch,
                time: Time::Eight,
            },
            Card {
                clock: ClockType::Watch,
                time: Time::Nine,
            },
            Card {
                clock: ClockType::Watch,
                time: Time::Ten,
            },
            Card {
                clock: ClockType::Watch,
                time: Time::ElevenThirty,
            },
            Card {
                clock: ClockType::Watch,
                time: Time::Twelve,
            },
            // atomic
            Card {
                clock: ClockType::Atomic,
                time: Time::Two,
            },
            Card {
                clock: ClockType::Atomic,
                time: Time::Three,
            },
            Card {
                clock: ClockType::Atomic,
                time: Time::Eight,
            },
            Card {
                clock: ClockType::Atomic,
                time: Time::Nine,
            },
            Card {
                clock: ClockType::Atomic,
                time: Time::Ten,
            },
            Card {
                clock: ClockType::Atomic,
                time: Time::Eleven,
            },
            Card {
                clock: ClockType::Atomic,
                time: Time::TwelveThirty,
            },
            // time machine
            Card {
                clock: ClockType::TimeMachine,
                time: Time::One,
            },
            Card {
                clock: ClockType::TimeMachine,
                time: Time::Three,
            },
            Card {
                clock: ClockType::TimeMachine,
                time: Time::Four,
            },
            Card {
                clock: ClockType::TimeMachine,
                time: Time::Five,
            },
            Card {
                clock: ClockType::TimeMachine,
                time: Time::Six,
            },
            Card {
                clock: ClockType::TimeMachine,
                time: Time::Seven,
            },
            Card {
                clock: ClockType::TimeMachine,
                time: Time::Eight,
            },
            Card {
                clock: ClockType::TimeMachine,
                time: Time::NineThirty,
            },
        ];
        let mut rng = rand::rng();
        card_pool.shuffle(&mut rng);

        Self(VecDeque::from(card_pool))
    }

    pub fn n_cards(&self) -> usize {
        self.0.len()
    }

    pub fn add_cards(&mut self, cards: impl Iterator<Item = Card>) {
        for card in cards {
            self.0.push_front(card);
        }
    }

    pub fn take_cards(&mut self, n_cards: usize) -> Vec<Card> {
        let mut cards = Vec::new();
        for _ in 0..n_cards.min(self.0.len()) {
            cards.push(self.0.pop_back().unwrap());
        }
        cards
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub clock: ClockType,
    pub time: Time,
}

impl Card {
    pub fn is_right_angle(&self) -> bool {
        let time_is_right = match self.time {
            Time::Three | Time::Nine => true,
            _ => false,
        };
        let clock_is_right = match self.clock {
            ClockType::Yellow
            | ClockType::Purple
            | ClockType::Watch
            | ClockType::Atomic
            | ClockType::TimeMachine => true,
            _ => false,
        };

        time_is_right && clock_is_right
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ClockType {
    Sun,
    Hourglass,
    Chinese,
    Yellow,
    Purple,
    Watch,
    Atomic,
    TimeMachine,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Time {
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
}

impl Time {
    pub const ALL: [Time; 24] = [
        Time::One,
        Time::OneThirty,
        Time::Two,
        Time::TwoThirty,
        Time::Three,
        Time::ThreeThirty,
        Time::Four,
        Time::FourThirty,
        Time::Five,
        Time::FiveThirty,
        Time::Six,
        Time::SixThirty,
        Time::Seven,
        Time::SevenThirty,
        Time::Eight,
        Time::EightThirty,
        Time::Nine,
        Time::NineThirty,
        Time::Ten,
        Time::TenThirty,
        Time::Eleven,
        Time::ElevenThirty,
        Time::Twelve,
        Time::TwelveThirty,
    ];

    pub fn get_index(&self) -> usize {
        match self {
            Time::One => 0,
            Time::OneThirty => 1,
            Time::Two => 2,
            Time::TwoThirty => 3,
            Time::Three => 4,
            Time::ThreeThirty => 5,
            Time::Four => 6,
            Time::FourThirty => 7,
            Time::Five => 8,
            Time::FiveThirty => 9,
            Time::Six => 10,
            Time::SixThirty => 11,
            Time::Seven => 12,
            Time::SevenThirty => 13,
            Time::Eight => 14,
            Time::EightThirty => 15,
            Time::Nine => 16,
            Time::NineThirty => 17,
            Time::Ten => 18,
            Time::TenThirty => 19,
            Time::Eleven => 20,
            Time::ElevenThirty => 21,
            Time::Twelve => 22,
            Time::TwelveThirty => 23,
        }
    }

    pub fn is_thirty(&self) -> bool {
        match self {
            Self::OneThirty
            | Self::TwoThirty
            | Self::ThreeThirty
            | Self::FourThirty
            | Self::FiveThirty
            | Self::SixThirty
            | Self::SevenThirty
            | Self::EightThirty
            | Self::NineThirty
            | Self::TenThirty
            | Self::ElevenThirty
            | Self::TwelveThirty => true,
            _ => false,
        }
    }
    pub fn plus(&self, time_interval: &TimeInterval) -> Self {
        let index = self.get_index();
        let new_index = (index as isize + time_interval.0).rem_euclid(24) as usize;
        Self::ALL[new_index]
    }
}

impl TryFrom<Count> for Time {
    type Error = ();

    fn try_from(value: Count) -> Result<Self, Self::Error> {
        Ok(match value {
            Count::One => Self::One,
            Count::OneThirty => Self::OneThirty,
            Count::Two => Self::Two,
            Count::TwoThirty => Self::TwoThirty,
            Count::Three => Self::Three,
            Count::ThreeThirty => Self::ThreeThirty,
            Count::Four => Self::Four,
            Count::FourThirty => Self::FourThirty,
            Count::Five => Self::Five,
            Count::FiveThirty => Self::FiveThirty,
            Count::Six => Self::Six,
            Count::SixThirty => Self::SixThirty,
            Count::Seven => Self::Seven,
            Count::SevenThirty => Self::SevenThirty,
            Count::Eight => Self::Eight,
            Count::EightThirty => Self::EightThirty,
            Count::Nine => Self::Nine,
            Count::NineThirty => Self::NineThirty,
            Count::Ten => Self::Ten,
            Count::TenThirty => Self::TenThirty,
            Count::Eleven => Self::Eleven,
            Count::ElevenThirty => Self::ElevenThirty,
            Count::Twelve => Self::Twelve,
            Count::TwelveThirty => Self::TwelveThirty,
            _ => {
                return Err(());
            }
        })
    }
}
