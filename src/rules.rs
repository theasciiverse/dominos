//! The rule sets, as data. Each follows a page on pagat.com, read
//! 2026-10-26; where the page offers variations, the main text is used.

use serde::{Deserialize, Serialize};

/// Which game is played.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Kind {
    Block,
    Draw,
    AllFives,
}

/// What a player may do about the boneyard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drawing {
    /// No drawing: the tiles not dealt stay out of the hand.
    None,
    /// Draw whenever you like, even holding a play, but never the last
    /// `keep` tiles (the Draw game).
    Free { keep: usize },
    /// Draw only when you cannot play, one tile at a time, until you can;
    /// a playable tile drawn must be played at once (All Fives).
    Forced,
}

/// Who leads the next hand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NextLead {
    /// The player to the left of the last leader.
    Rotate,
    /// Whoever dominoed; after a blocked hand, by lot.
    Winner,
}

/// How a hand is scored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HandScore {
    /// The lowest count wins the losers' pips less their own; a tie for
    /// lowest scores nothing.
    LowestTakesDifference,
    /// The winner takes the opponents' pips rounded to the nearest five
    /// (All Fives).
    OpponentsRounded,
}

#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub kind: Kind,
    pub name: &'static str,
    /// One line on the game.
    pub blurb: &'static str,
    /// Where the rules come from.
    pub source: &'static str,
    /// Tiles each player is dealt, for 2, 3 and 4 players.
    pub deal: [usize; 3],
    pub drawing: Drawing,
    /// The first double is a spinner, played on all four sides.
    pub spinner: bool,
    /// Multiples of five on the ends score as they are made.
    pub score_ends: bool,
    pub hand_score: HandScore,
    pub next_lead: NextLead,
    /// The target for 2 players, and for 3 or 4.
    pub target: [u32; 2],
}

pub const BLOCK: Rules = Rules {
    kind: Kind::Block,
    name: "Block game",
    blurb: "The plainest game: match the ends, pass when you cannot.",
    source: "pagat.com, Block Dominoes (John McLeod)",
    deal: [7, 5, 5],
    drawing: Drawing::None,
    spinner: false,
    score_ends: false,
    hand_score: HandScore::LowestTakesDifference,
    next_lead: NextLead::Rotate,
    target: [100, 61],
};

pub const DRAW: Rules = Rules {
    kind: Kind::Draw,
    name: "Draw game",
    blurb: "The Block game with a boneyard to draw from, the last two left.",
    source: "pagat.com, Draw Dominoes (John McLeod)",
    deal: [7, 7, 6],
    drawing: Drawing::Free { keep: 2 },
    spinner: false,
    score_ends: false,
    hand_score: HandScore::LowestTakesDifference,
    next_lead: NextLead::Rotate,
    target: [100, 61],
};

pub const ALL_FIVES: Rules = Rules {
    kind: Kind::AllFives,
    name: "All Fives",
    blurb: "Score when the open ends add up to a multiple of five; a spinner.",
    source: "pagat.com, All Fives (John McLeod)",
    deal: [9, 7, 5],
    drawing: Drawing::Forced,
    spinner: true,
    score_ends: true,
    hand_score: HandScore::OpponentsRounded,
    next_lead: NextLead::Winner,
    target: [250, 200],
};

pub const ALL: [Rules; 3] = [BLOCK, DRAW, ALL_FIVES];

pub fn of(kind: Kind) -> Rules {
    match kind {
        Kind::Block => BLOCK,
        Kind::Draw => DRAW,
        Kind::AllFives => ALL_FIVES,
    }
}

impl Rules {
    pub fn hand_size(&self, players: usize) -> usize {
        self.deal[players.clamp(2, 4) - 2]
    }

    /// The target: the two-player one also for partners in the Block
    /// and Draw games (pagat: "100 points in a two player or partnership
    /// game"); All Fives is 200 for three or four, partners or not.
    pub fn target(&self, players: usize, teams: bool) -> u32 {
        if players == 2 || (teams && self.kind != Kind::AllFives) {
            self.target[0]
        } else {
            self.target[1]
        }
    }
}

/// `n` rounded to the nearest five: 3 is 5, 2 is 0.
pub fn round_to_five(n: u32) -> u32 {
    (n + 2) / 5 * 5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_follows_pagats_examples() {
        assert_eq!(round_to_five(3), 5); // [1-2]
        assert_eq!(round_to_five(2), 0); // [1-1]
        assert_eq!(round_to_five(22), 20);
        assert_eq!(round_to_five(23), 25);
        assert_eq!(round_to_five(25), 25);
    }

    #[test]
    fn no_deal_needs_more_than_the_set() {
        for r in ALL {
            for n in 2..=4 {
                assert!(r.hand_size(n) * n <= 28, "{} {n}", r.name);
            }
        }
    }
}
