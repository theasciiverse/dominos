//! The layout on the table: one tile in the middle and the arms that grow
//! from it.
//!
//! Without a spinner the layout is a line, the root tile with a Left and a
//! Right arm. With one (All Fives: the first double played), the spinner
//! itself is made the root, so its four sides are the four arms. When the
//! first double lands at the end of an arm, the line is turned around it:
//! the tiles between it and the old root, the old root, and the other arm
//! become the spinner's arm on the opposite side.

use crate::tile::Tile;
use serde::{Deserialize, Serialize};

/// One of the four directions a layout can grow in.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Arm {
    Left,
    Right,
    Up,
    Down,
}

impl Arm {
    pub const ALL: [Arm; 4] = [Arm::Left, Arm::Right, Arm::Up, Arm::Down];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Arm::Left => "left",
            Arm::Right => "right",
            Arm::Up => "top",
            Arm::Down => "bottom",
        }
    }
}

/// A tile in an arm: `inner` touches the tile before it, `outer` is open
/// (or touches the tile after it).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Link {
    pub inner: u8,
    pub outer: u8,
}

impl Link {
    pub fn is_double(self) -> bool {
        self.inner == self.outer
    }

    pub fn tile(self) -> Tile {
        Tile::new(self.inner, self.outer)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Board {
    /// The middle tile, as its left and right numbers.
    root: Option<(u8, u8)>,
    /// Whether the root is a spinner.
    spinner: bool,
    /// Whether the rules make the first double a spinner.
    spinners: bool,
    arms: [Vec<Link>; 4],
}

impl Board {
    pub fn new(spinners: bool) -> Board {
        Board {
            spinners,
            ..Board::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn root(&self) -> Option<(u8, u8)> {
        self.root
    }

    #[cfg(test)]
    pub fn has_spinner(&self) -> bool {
        self.spinner
    }

    pub fn arm(&self, arm: Arm) -> &[Link] {
        &self.arms[arm.index()]
    }

    /// Every tile on the table.
    pub fn tiles(&self) -> Vec<Tile> {
        let mut all: Vec<Tile> = self.root.iter().map(|&(l, r)| Tile::new(l, r)).collect();
        for arm in &self.arms {
            all.extend(arm.iter().map(|l| l.tile()));
        }
        all
    }

    pub fn len(&self) -> usize {
        self.root.map_or(0, |_| 1) + self.arms.iter().map(Vec::len).sum::<usize>()
    }

    /// The number open at the end of `arm`, if a tile can go there.
    pub fn open(&self, arm: Arm) -> Option<u8> {
        let (l, r) = self.root?;
        let tiles = &self.arms[arm.index()];
        if let Some(last) = tiles.last() {
            return Some(last.outer);
        }
        match arm {
            Arm::Left => Some(l),
            Arm::Right => Some(r),
            // The spinner's long sides open once both its ends are covered.
            Arm::Up | Arm::Down => {
                let covered = !self.arms[Arm::Left.index()].is_empty()
                    && !self.arms[Arm::Right.index()].is_empty();
                (self.spinner && covered).then_some(l)
            }
        }
    }

    /// The open ends, with their numbers.
    pub fn ends(&self) -> Vec<(Arm, u8)> {
        Arm::ALL
            .iter()
            .filter_map(|&a| self.open(a).map(|n| (a, n)))
            .collect()
    }

    /// Where `tile` can go. On an empty table, only the lead (`Left`).
    pub fn places(&self, tile: Tile) -> Vec<Arm> {
        if self.is_empty() {
            return vec![Arm::Left];
        }
        self.ends()
            .into_iter()
            .filter(|&(_, n)| tile.has(n))
            .map(|(a, _)| a)
            .collect()
    }

    pub fn fits(&self, tile: Tile) -> bool {
        !self.places(tile).is_empty()
    }

    /// Lay `tile` at the end of `arm`. The caller has checked it fits.
    pub fn play(&mut self, tile: Tile, arm: Arm) {
        if self.root.is_none() {
            self.root = Some((tile.lo, tile.hi));
            self.spinner = self.spinners && tile.is_double();
            return;
        }
        let n = self.open(arm).expect("an open end");
        let outer = tile.other(n).expect("a tile that fits");
        self.arms[arm.index()].push(Link { inner: n, outer });
        if self.spinners && !self.spinner && tile.is_double() {
            self.turn_around(arm);
        }
    }

    /// Make the double just laid at the end of `arm` the root.
    fn turn_around(&mut self, arm: Arm) {
        let (l, r) = self.root.expect("a root");
        let mut path = std::mem::take(&mut self.arms[arm.index()]);
        let double = path.pop().expect("the double");
        let other = match arm {
            Arm::Left => Arm::Right,
            _ => Arm::Left,
        };
        let rest = std::mem::take(&mut self.arms[other.index()]);
        let mut inward: Vec<Link> = path
            .iter()
            .rev()
            .map(|t| Link {
                inner: t.outer,
                outer: t.inner,
            })
            .collect();
        inward.push(match arm {
            Arm::Left => Link { inner: l, outer: r },
            _ => Link { inner: r, outer: l },
        });
        inward.extend(rest);
        self.root = Some((double.inner, double.inner));
        self.spinner = true;
        self.arms[other.index()] = inward;
    }

    /// The count of the open ends, as All Fives scores it: a double at an
    /// end counts both halves, a spinner counts until both its ends are
    /// covered, and an arm not yet started counts nothing.
    pub fn count(&self) -> u32 {
        let Some((l, r)) = self.root else { return 0 };
        if self.arms.iter().all(Vec::is_empty) {
            return (l + r) as u32;
        }
        let root_double = l == r;
        let mut sum = 0u32;
        for arm in Arm::ALL {
            let n = match self.arms[arm.index()].last() {
                Some(t) if t.is_double() => 2 * t.outer,
                Some(t) => t.outer,
                None => match arm {
                    Arm::Left if root_double => 2 * l,
                    Arm::Left => l,
                    Arm::Right if root_double => 2 * r,
                    Arm::Right => r,
                    _ => 0,
                },
            };
            sum += n as u32;
        }
        sum
    }

    /// The points a play just made: the count, if it is a multiple of five.
    pub fn score(&self) -> u32 {
        let c = self.count();
        if c.is_multiple_of(5) { c } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(a: u8, b: u8) -> Tile {
        Tile::new(a, b)
    }

    #[test]
    fn a_line_matches_at_both_ends() {
        let mut b = Board::new(false);
        b.play(t(3, 5), Arm::Left);
        assert_eq!(b.ends(), vec![(Arm::Left, 3), (Arm::Right, 5)]);
        assert_eq!(b.places(t(5, 5)), vec![Arm::Right]);
        b.play(t(5, 5), Arm::Right);
        // no spinner in a line game: the double is just crosswise
        assert!(!b.has_spinner());
        assert_eq!(b.ends(), vec![(Arm::Left, 3), (Arm::Right, 5)]);
        assert_eq!(b.places(t(3, 5)), vec![Arm::Left, Arm::Right]);
    }

    #[test]
    fn a_double_led_is_the_spinner() {
        let mut b = Board::new(true);
        b.play(t(5, 5), Arm::Left);
        assert_eq!(b.count(), 10);
        b.play(t(5, 3), Arm::Left);
        // the spinner still counts both halves until its other end is covered
        assert_eq!(b.count(), 13);
        assert_eq!(b.ends().len(), 2);
        b.play(t(5, 0), Arm::Right);
        assert_eq!(b.count(), 3);
        // now its long sides open
        assert_eq!(b.ends().len(), 4);
        b.play(t(5, 2), Arm::Up);
        assert_eq!(b.count(), 5);
        b.play(t(5, 6), Arm::Down);
        assert_eq!(b.count(), 11);
    }

    #[test]
    fn pagats_highest_play_counts_twenty() {
        let mut b = Board::new(false);
        b.play(t(6, 4), Arm::Left);
        b.play(t(4, 4), Arm::Left);
        b.play(t(6, 6), Arm::Right);
        assert_eq!(b.count(), 20);
    }

    #[test]
    fn the_first_double_becomes_the_root() {
        let mut b = Board::new(true);
        b.play(t(2, 4), Arm::Left); // 2 left, 4 right
        b.play(t(4, 1), Arm::Right);
        b.play(t(0, 2), Arm::Left);
        b.play(t(1, 1), Arm::Right);
        assert!(b.has_spinner());
        assert_eq!(b.root(), Some((1, 1)));
        assert!(b.arm(Arm::Right).is_empty());
        let inward: Vec<(u8, u8)> = b
            .arm(Arm::Left)
            .iter()
            .map(|l| (l.inner, l.outer))
            .collect();
        assert_eq!(inward, vec![(1, 4), (4, 2), (2, 0)]);
        // the spinner at an end counts both halves: 2 + 0
        assert_eq!(b.count(), 2);
        assert_eq!(b.ends(), vec![(Arm::Left, 0), (Arm::Right, 1)]);
        b.play(t(1, 5), Arm::Right);
        assert_eq!(b.ends().len(), 4);
        assert_eq!(b.count(), 5);
        // a second double is not a spinner
        b.play(t(5, 5), Arm::Right);
        assert_eq!(b.root(), Some((1, 1)));
        assert_eq!(b.count(), 10);
    }

    #[test]
    fn turning_from_the_left_arm() {
        let mut b = Board::new(true);
        b.play(t(2, 4), Arm::Left);
        b.play(t(4, 6), Arm::Right);
        b.play(t(2, 2), Arm::Left);
        assert_eq!(b.root(), Some((2, 2)));
        assert!(b.arm(Arm::Left).is_empty());
        let inward: Vec<(u8, u8)> = b
            .arm(Arm::Right)
            .iter()
            .map(|l| (l.inner, l.outer))
            .collect();
        assert_eq!(inward, vec![(2, 4), (4, 6)]);
        assert_eq!(b.len(), 3);
    }
}
