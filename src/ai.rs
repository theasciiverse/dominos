//! The computer player, at three strengths. It looks only at what its
//! seat can see: its own hand, the table, how many tiles the others hold,
//! and which numbers they have shown they lack.

use crate::board::Board;
use crate::game::{Action, Game, legal};
use crate::rng::Rng;
use crate::rules::Kind;
use crate::tile::Tile;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Strength {
    Easy,
    Normal,
    Hard,
}

impl Strength {
    pub const ALL: [Strength; 3] = [Strength::Easy, Strength::Normal, Strength::Hard];

    pub fn name(self) -> &'static str {
        match self {
            Strength::Easy => "easy",
            Strength::Normal => "normal",
            Strength::Hard => "hard",
        }
    }
}

pub fn choose(game: &Game, strength: Strength, rng: &mut Rng) -> Action {
    let seat = game.turn;
    let hand = &game.hands[seat];
    let actions = legal(
        &game.rules,
        &game.board,
        hand,
        game.boneyard.len(),
        game.must_play,
    );
    let plays: Vec<Action> = actions
        .iter()
        .copied()
        .filter(|a| matches!(a, Action::Play(..)))
        .collect();
    if plays.is_empty() {
        // nothing to weigh: draw or pass
        return actions[0];
    }
    if strength == Strength::Easy {
        return plays[rng.below(plays.len())];
    }
    let mut best = plays[0];
    let mut best_value = f64::MIN;
    for &action in &plays {
        // a little noise breaks ties between equal plays
        let value = weigh(game, seat, action, strength) + rng.below(100) as f64 * 1e-4;
        if value > best_value {
            best_value = value;
            best = action;
        }
    }
    best
}

/// How good a play looks.
fn weigh(game: &Game, seat: usize, action: Action, strength: Strength) -> f64 {
    let Action::Play(tile, arm) = action else {
        return 0.0;
    };
    let mut board = game.board.clone();
    board.play(tile, arm);
    let scored = if game.rules.score_ends {
        board.score()
    } else {
        0
    } as f64;
    let rest: Vec<Tile> = game.hands[seat]
        .iter()
        .copied()
        .filter(|&t| t != tile)
        .collect();
    if rest.is_empty() {
        return 1000.0 + scored;
    }
    // shed weight: heavy tiles count against you at the end
    let mut value = tile.pips() as f64 + scored * 4.0;
    if strength == Strength::Normal {
        return value;
    }
    // doubles are hard to place later
    if tile.is_double() {
        value += 3.0;
    }
    let ends: Vec<u8> = board.ends().into_iter().map(|(_, n)| n).collect();
    // keep a play for next turn
    let follow = rest
        .iter()
        .filter(|t| ends.iter().any(|&n| t.has(n)))
        .count();
    value += if follow == 0 {
        -6.0
    } else {
        (follow as f64).min(3.0)
    };
    let next = (seat + 1) % game.players;
    let next_void = ends.iter().all(|&n| game.voids[next][n as usize]);
    if game.rules.kind == Kind::AllFives {
        value -= reply(game, seat, next, &board) * 3.0;
    } else if next_void {
        // they must pass (or draw)
        value += 6.0;
    }
    value
}

/// What the next player can be expected to score in reply: each tile
/// they might hold, best reply first, weighted by the chance it is the
/// best one they have.
fn reply(game: &Game, seat: usize, next: usize, board: &Board) -> f64 {
    let unseen = game.unseen(seat);
    let possible: Vec<Tile> = unseen
        .into_iter()
        .filter(|t| !game.voids[next][t.lo as usize] && !game.voids[next][t.hi as usize])
        .collect();
    if possible.is_empty() {
        return 0.0;
    }
    let held = game.hands[next].len() as f64;
    let chance = (held / possible.len() as f64).min(1.0);
    let mut scores: Vec<u32> = possible
        .iter()
        .map(|&t| {
            board
                .places(t)
                .into_iter()
                .map(|arm| {
                    let mut b = board.clone();
                    b.play(t, arm);
                    b.score()
                })
                .max()
                .unwrap_or(0)
        })
        .filter(|&s| s > 0)
        .collect();
    scores.sort_unstable_by(|a, b| b.cmp(a));
    let mut expected = 0.0;
    let mut missed = 1.0;
    for s in scores {
        expected += s as f64 * chance * missed;
        missed *= 1.0 - chance;
    }
    expected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Phase;
    use crate::rules::{ALL, Rules};

    /// Wins for seat 0 playing `a` against seat 1 playing `b`, seats
    /// swapped every other game.
    fn duel(rules: Rules, a: Strength, b: Strength, games: u64) -> u64 {
        let mut wins = 0;
        for seed in 0..games {
            let swap = seed % 2 == 1;
            let (mut g, _) = Game::new(rules, 2, seed);
            let mut rng = Rng::new(seed ^ 0xD0);
            loop {
                match g.phase {
                    Phase::Playing => {
                        let first = (g.turn == 0) != swap;
                        let action = choose(&g, if first { a } else { b }, &mut rng);
                        g.apply(action).unwrap();
                    }
                    Phase::HandOver(_) => {
                        g.next_hand();
                    }
                    Phase::GameOver { winner } => {
                        if (winner == 0) != swap {
                            wins += 1;
                        }
                        break;
                    }
                }
            }
        }
        wins
    }

    #[test]
    #[ignore = "slow; run with --ignored --nocapture to measure"]
    fn strengths() {
        for rules in ALL {
            let n = 400;
            let ne = duel(rules, Strength::Normal, Strength::Easy, n);
            let he = duel(rules, Strength::Hard, Strength::Easy, n);
            let hn = duel(rules, Strength::Hard, Strength::Normal, n);
            println!(
                "{}: normal beats easy {ne}/{n}, hard beats easy {he}/{n}, hard beats normal {hn}/{n}",
                rules.name
            );
        }
    }

    #[test]
    fn hard_beats_easy() {
        for rules in ALL {
            let wins = duel(rules, Strength::Hard, Strength::Easy, 60);
            assert!(wins > 30, "{}: {wins} of 60", rules.name);
        }
    }
}
