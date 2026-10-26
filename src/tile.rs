//! The tiles of a double-six set.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The highest number on a tile: a double-six set.
pub const TOP: u8 = 6;

/// A tile, stored with its smaller number first, so two tiles with the
/// same numbers are equal however they are turned.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct Tile {
    pub lo: u8,
    pub hi: u8,
}

impl Tile {
    pub fn new(a: u8, b: u8) -> Tile {
        Tile {
            lo: a.min(b),
            hi: a.max(b),
        }
    }

    pub fn is_double(self) -> bool {
        self.lo == self.hi
    }

    pub fn pips(self) -> u32 {
        (self.lo + self.hi) as u32
    }

    pub fn has(self, n: u8) -> bool {
        self.lo == n || self.hi == n
    }

    /// The number at the other end from `n`, if the tile has `n`.
    pub fn other(self, n: u8) -> Option<u8> {
        if self.lo == n {
            Some(self.hi)
        } else if self.hi == n {
            Some(self.lo)
        } else {
            None
        }
    }

    /// All 28 tiles of the set, in order.
    pub fn set() -> Vec<Tile> {
        let mut all = Vec::with_capacity(28);
        for lo in 0..=TOP {
            for hi in lo..=TOP {
                all.push(Tile { lo, hi });
            }
        }
        all
    }
}

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}|{}]", self.lo, self.hi)
    }
}

/// The pips in a hand.
pub fn pips(hand: &[Tile]) -> u32 {
    hand.iter().map(|t| t.pips()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_set_has_28_tiles_and_168_pips() {
        let set = Tile::set();
        assert_eq!(set.len(), 28);
        assert_eq!(pips(&set), 168);
        assert_eq!(set.iter().filter(|t| t.is_double()).count(), 7);
    }

    #[test]
    fn turned_tiles_are_equal() {
        assert_eq!(Tile::new(5, 2), Tile::new(2, 5));
        assert_eq!(Tile::new(5, 2).other(5), Some(2));
        assert_eq!(Tile::new(5, 2).other(3), None);
    }
}
