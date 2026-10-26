//! Where each tile of the layout goes on the screen.
//!
//! Every arm runs straight out from the root and, at the edge of the
//! table, turns clockwise: the right arm turns down, the bottom one left,
//! the left one up and the top one right, a pinwheel that keeps the arms
//! apart. Doubles lie across the arm. If the large tiles do not fit, the
//! small ones are tried.

use crate::board::{Arm, Board, Link};
use crate::tile::Tile;

/// The tiles' size on screen; widths and heights are odd so a tile always
/// has a middle cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Size {
    /// A tile lying across: width, height.
    pub across: (i32, i32),
    /// A tile standing up: width, height.
    pub upright: (i32, i32),
    pub gap: (i32, i32),
}

pub const LARGE: Size = Size {
    across: (11, 3),
    upright: (5, 7),
    gap: (1, 1),
};
pub const SMALL: Size = Size {
    across: (5, 1),
    upright: (3, 3),
    gap: (1, 0),
};

impl Size {
    pub fn is_large(self) -> bool {
        self == LARGE
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dir {
    East,
    South,
    West,
    North,
}

impl Dir {
    fn clockwise(self) -> Dir {
        match self {
            Dir::East => Dir::South,
            Dir::South => Dir::West,
            Dir::West => Dir::North,
            Dir::North => Dir::East,
        }
    }

    fn anticlockwise(self) -> Dir {
        self.clockwise().clockwise().clockwise()
    }

    fn across(self) -> bool {
        matches!(self, Dir::East | Dir::West)
    }
}

/// A tile on the screen.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Standing up (top and bottom halves) or lying across (left, right).
    pub upright: bool,
    /// The number on the top or left half, then the bottom or right.
    pub first: u8,
    pub second: u8,
    /// A tile not yet played, shown where it would go.
    pub ghost: Option<Arm>,
    /// Which way its arm was running, for the next tile.
    dir: Dir,
    double: bool,
}

impl Placed {
    pub fn tile(&self) -> Tile {
        Tile::new(self.first, self.second)
    }

    fn centre(&self) -> (i32, i32) {
        (self.x + (self.w - 1) / 2, self.y + (self.h - 1) / 2)
    }

    /// The middle of the half facing out along the arm.
    fn outer_centre(&self) -> (i32, i32) {
        let (cx, cy) = self.centre();
        if self.double {
            return (cx, cy);
        }
        let half_w = (self.w - 1) / 2;
        let half_h = (self.h - 1) / 2;
        match self.dir {
            Dir::East => (self.x + self.w - 1 - half_w / 2, cy),
            Dir::West => (self.x + half_w / 2, cy),
            Dir::South => (cx, self.y + self.h - 1 - half_h / 2),
            Dir::North => (cx, self.y + half_h / 2),
        }
    }

    fn crowds(&self, other: &Placed, gap: (i32, i32)) -> bool {
        self.x < other.x + other.w + gap.0
            && other.x < self.x + self.w + gap.0
            && self.y < other.y + other.h + gap.1
            && other.y < self.y + self.h + gap.1
    }
}

pub struct Layout {
    pub size: Size,
    pub tiles: Vec<Placed>,
}

/// Lay out `board` in a table `width` by `height`, with `ghosts`: tiles
/// shown at the ends they could be played at.
pub fn layout(board: &Board, ghosts: &[(Arm, Tile)], width: i32, height: i32) -> Layout {
    for size in [LARGE, SMALL] {
        for (dx, dy) in offsets(board, ghosts, size) {
            if let Some(tiles) = place(board, ghosts, width, height, size, (dx, dy), true) {
                return Layout { size, tiles };
            }
        }
    }
    // Nothing fits: draw it anyway, cut off at the edges.
    let tiles = place(board, ghosts, width, height, SMALL, (0, 0), false).unwrap_or_default();
    Layout { size: SMALL, tiles }
}

/// Where to try the root, nearest the middle first, leaning away from
/// the longer arms.
fn offsets(board: &Board, ghosts: &[(Arm, Tile)], size: Size) -> Vec<(i32, i32)> {
    let len =
        |arm: Arm| (board.arm(arm).len() + ghosts.iter().filter(|g| g.0 == arm).count()) as i32;
    let step_x = size.across.0 + size.gap.0;
    let step_y = size.upright.1 + size.gap.1;
    let lean_x = (len(Arm::Left) - len(Arm::Right)) * step_x / 2;
    let lean_y = (len(Arm::Up) - len(Arm::Down)) * step_y / 2;
    let mut all = Vec::new();
    for fy in [1, 0] {
        for fx in [2, 1, 0, 3, 4] {
            all.push((lean_x * fx / 2, lean_y * fy));
        }
    }
    all
}

fn place(
    board: &Board,
    ghosts: &[(Arm, Tile)],
    width: i32,
    height: i32,
    size: Size,
    offset: (i32, i32),
    strict: bool,
) -> Option<Vec<Placed>> {
    let Some((l, r)) = board.root() else {
        // a lead to come: the ghost alone, in the middle
        return Some(
            ghosts
                .iter()
                .map(|&(arm, t)| {
                    let mut p = make(
                        size,
                        Dir::East,
                        Link {
                            inner: t.lo,
                            outer: t.hi,
                        },
                    );
                    p.x = (width - p.w) / 2;
                    p.y = (height - p.h) / 2;
                    p.ghost = Some(arm);
                    p
                })
                .collect(),
        );
    };
    let mut root = make(size, Dir::East, Link { inner: l, outer: r });
    root.x = (width - root.w) / 2 + offset.0;
    root.y = (height - root.h) / 2 + offset.1;
    let fits = |p: &Placed| p.x >= 0 && p.y >= 0 && p.x + p.w <= width && p.y + p.h <= height;
    if strict && !fits(&root) {
        return None;
    }
    let mut placed = vec![root];
    for arm in Arm::ALL {
        let mut links: Vec<(Link, bool)> = board.arm(arm).iter().map(|&l| (l, false)).collect();
        if let Some(open) = board.open(arm) {
            for &(a, t) in ghosts {
                if a == arm {
                    let outer = t.other(open).unwrap_or(open);
                    links.push((Link { inner: open, outer }, true));
                }
            }
        }
        let mut dir = match arm {
            Arm::Left => Dir::West,
            Arm::Right => Dir::East,
            Arm::Up => Dir::North,
            Arm::Down => Dir::South,
        };
        let mut prev = root;
        for (link, ghost) in links {
            let mut next = attach(size, &prev, dir, link, false);
            if strict && !fits(&next) {
                let turns = [dir.clockwise(), dir.anticlockwise()];
                let turned = turns
                    .iter()
                    .map(|&d| attach(size, &prev, d, link, true))
                    .find(|p| fits(p) && !placed.iter().any(|q| q.crowds(p, size.gap)));
                next = turned?;
                dir = next.dir;
            }
            if strict && placed.iter().any(|q| q.crowds(&next, size.gap)) {
                return None;
            }
            if ghost {
                next.ghost = Some(arm);
            }
            placed.push(next);
            prev = next;
        }
    }
    Some(placed)
}

/// A tile on its own, running `dir`, at the origin.
fn make(size: Size, dir: Dir, link: Link) -> Placed {
    let double = link.is_double();
    // doubles lie across their arm
    let upright = dir.across() == double;
    let (w, h) = if upright { size.upright } else { size.across };
    let (first, second) = match dir {
        Dir::East | Dir::South => (link.inner, link.outer),
        Dir::West | Dir::North => (link.outer, link.inner),
    };
    Placed {
        x: 0,
        y: 0,
        w,
        h,
        upright,
        first,
        second,
        ghost: None,
        dir,
        double,
    }
}

/// The tile after `prev`, running `dir`; `turned` when the arm has just
/// changed direction, so the new tile sits against the outer half.
fn attach(size: Size, prev: &Placed, dir: Dir, link: Link, turned: bool) -> Placed {
    let mut p = make(size, dir, link);
    let (cx, cy) = if turned {
        prev.outer_centre()
    } else {
        prev.centre()
    };
    match dir {
        Dir::East => p.x = prev.x + prev.w + size.gap.0,
        Dir::West => p.x = prev.x - size.gap.0 - p.w,
        Dir::South => p.y = prev.y + prev.h + size.gap.1,
        Dir::North => p.y = prev.y - size.gap.1 - p.h,
    }
    if dir.across() {
        p.y = cy - (p.h - 1) / 2;
    } else {
        p.x = cx - (p.w - 1) / 2;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Game, Phase};
    use crate::rng::Rng;
    use crate::rules::ALL;

    fn no_overlaps(l: &Layout) {
        for (i, a) in l.tiles.iter().enumerate() {
            for b in &l.tiles[i + 1..] {
                assert!(!a.crowds(b, (0, 0)), "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn neighbours_touch_with_matching_numbers() {
        let mut b = Board::new(false);
        b.play(Tile::new(3, 5), Arm::Left);
        b.play(Tile::new(5, 1), Arm::Right);
        b.play(Tile::new(3, 3), Arm::Left);
        let l = layout(&b, &[], 80, 20);
        assert_eq!(l.size, LARGE);
        let row: Vec<(u8, u8)> = {
            let mut t = l.tiles.clone();
            t.sort_by_key(|p| p.x);
            t.iter().map(|p| (p.first, p.second)).collect()
        };
        assert_eq!(row, vec![(3, 3), (3, 5), (5, 1)]);
        assert!(
            l.tiles
                .iter()
                .find(|p| p.first == 3 && p.second == 3)
                .unwrap()
                .upright
        );
    }

    #[test]
    fn whole_games_lay_out_without_overlaps() {
        for rules in ALL {
            for players in 2..=4 {
                for seed in 0..10 {
                    let (mut g, _) = Game::new(rules, players, false, seed);
                    let mut rng = Rng::new(seed);
                    while matches!(g.phase, Phase::Playing) {
                        let legal = g.legal();
                        g.apply(legal[rng.below(legal.len())]).unwrap();
                        for (w, h) in [(100, 18), (80, 12)] {
                            let l = layout(&g.board, &[], w, h);
                            assert_eq!(l.tiles.len(), g.board.len());
                            no_overlaps(&l);
                        }
                    }
                }
            }
        }
    }
}
