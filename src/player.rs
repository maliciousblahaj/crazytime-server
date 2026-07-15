use crate::card::{Card, Time};

/// corresponds exactly to the session id of the user, used to authenticate
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct PlayerId(u128);

/// the public id everyone in the lobby knows
pub struct PublicPlayerId(usize);

impl PlayerId {
    pub fn new() -> Self {
        Self(rand::random())
    }
}

/// per lobby player state
pub struct PlayerState {
    pub id: PlayerId,
    pub public_id: usize,
    /// the private card hand, not the stack they've revealed
    pub card_hand: Vec<Card>,
}

impl PlayerState {
    pub fn new(id: PlayerId, public_id: usize) -> Self {
        Self {
            id,
            public_id,
            card_hand: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
pub enum ValidPlayerMove {
    CountAndLayCard { card: Card, count: Time },
    Count(Time),
    LayCard(Card),
}

// im just writing this here to reason about the representation.
//
// so every single player knows everything as every other player. all information
// someone knows is public, and will be continuously updated from every server message.
// If a user performs an action the server will respond by broadcasting their action
// to everyone, including themselves, and only then should the ui update.
//
// what do the criterias know? the criterias know literally exactly all information that
// is public, and has been accumulated from all actions. this means the server must keep
// all state that is accumulated to the same degree as the frontend will do, else the
// frontend might know more than the server does. And the server will update its internal
// "frontend" of public information when sending every action, just like the frontend
// when it receives every action, and that internal state update could literally be a
// function which takes in a borrowed servermessage and processes it to update internal
// state, like a mock frontend.
//
// in this same mock frontend is also where the expected things are stored, since that *is*
// public knowledge, just not explicit, and its literally the entire gameplay to keep track
// of this. here we also store if an error happened. we only need public information to know
// this.
//
// in fact we never need private information for anything, the only private thing is each
// players' card hands, which noone needs to know, and can almost be abstracted away as a
// random pool.
