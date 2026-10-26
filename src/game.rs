//! The game: deals, turns, scoring. Every change goes through `apply`, an
//! action in and events out. Over a network the host runs the game and
//! sends each player their `View` and the events, with the tiles others
//! draw hidden.
//!
//! Seats go round to the left: seat 1 sits on seat 0's left, and so on.
//! Partners (four players only) sit opposite: seats 0 and 2 are side 0,
//! seats 1 and 3 side 1. Without partners each seat is its own side.
//! Scores are kept by side.

use crate::board::{Arm, Board};
use crate::rng::Rng;
use crate::rules::{Drawing, HandScore, NextLead, Rules, round_to_five};
use crate::tile::{self, TOP, Tile};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Action {
    Play(Tile, Arm),
    Draw,
    Pass,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    /// A new hand is dealt and `who` leads.
    Dealt {
        who: usize,
        by_lot: bool,
    },
    Played {
        who: usize,
        tile: Tile,
        arm: Arm,
        scored: u32,
    },
    /// The tile is known only to the one who drew it.
    Drew {
        who: usize,
        tile: Option<Tile>,
    },
    Passed {
        who: usize,
    },
    HandOver(HandResult),
    /// The side that won.
    GameOver {
        winner: usize,
    },
}

impl Event {
    /// The event as `seat` may see it.
    pub fn for_seat(&self, seat: usize) -> Event {
        match self {
            Event::Drew { who, .. } if *who != seat => Event::Drew {
                who: *who,
                tile: None,
            },
            e => e.clone(),
        }
    }
}

/// How a hand ended and what it scored.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandResult {
    /// The hands as they were at the end, and their pips, by seat.
    pub hands: Vec<Vec<Tile>>,
    pub pips: Vec<u32>,
    /// Who played out, if anyone did.
    pub dominoed: Option<usize>,
    /// The side that won the hand; `None` when tied for lowest.
    pub winner: Option<usize>,
    /// The points each side gets for the hand.
    pub points: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Phase {
    Playing,
    /// Waiting for the next deal.
    HandOver(HandResult),
    GameOver {
        winner: usize,
    },
}

#[derive(Clone, Debug)]
pub struct Game {
    pub rules: Rules,
    pub players: usize,
    /// Partners sit opposite (four players only).
    pub teams: bool,
    pub hands: Vec<Vec<Tile>>,
    pub boneyard: Vec<Tile>,
    pub board: Board,
    pub turn: usize,
    pub leader: usize,
    /// By side.
    pub scores: Vec<u32>,
    pub phase: Phase,
    /// Numbers each player has been seen to lack this hand, from their
    /// passes and forced draws: public knowledge, which the computer uses.
    pub voids: Vec<[bool; TOP as usize + 1]>,
    /// A tile just drawn that must be played at once (All Fives).
    pub must_play: Option<Tile>,
    /// Passes in a row; a full round of them blocks the hand.
    passes: usize,
    pub hand_no: u32,
    rng: Rng,
}

/// What a player may do, from what they can see.
pub fn legal(
    rules: &Rules,
    board: &Board,
    hand: &[Tile],
    boneyard: usize,
    must_play: Option<Tile>,
) -> Vec<Action> {
    let mut plays = Vec::new();
    for &tile in hand {
        if must_play.is_some_and(|m| m != tile) {
            continue;
        }
        for arm in board.places(tile) {
            plays.push(Action::Play(tile, arm));
        }
    }
    if board.is_empty() {
        return plays;
    }
    match rules.drawing {
        Drawing::None => {}
        Drawing::Free { keep } => {
            if boneyard > keep {
                plays.push(Action::Draw);
            }
        }
        Drawing::Forced => {
            if plays.is_empty() && boneyard > 0 {
                plays.push(Action::Draw);
            }
        }
    }
    if plays.is_empty() {
        plays.push(Action::Pass);
    }
    plays
}

impl Game {
    /// A new game; the first lead is by lot.
    pub fn new(rules: Rules, players: usize, teams: bool, seed: u64) -> (Game, Vec<Event>) {
        let teams = teams && players == 4;
        let mut game = Game {
            rules,
            players,
            teams,
            hands: vec![Vec::new(); players],
            boneyard: Vec::new(),
            board: Board::new(rules.spinner),
            turn: 0,
            leader: 0,
            scores: vec![0; if teams { 2 } else { players }],
            phase: Phase::Playing,
            voids: vec![[false; TOP as usize + 1]; players],
            must_play: None,
            passes: 0,
            hand_no: 0,
            rng: Rng::new(seed),
        };
        let leader = game.rng.below(players);
        let events = game.deal(leader, true);
        (game, events)
    }

    fn deal(&mut self, leader: usize, by_lot: bool) -> Vec<Event> {
        let mut set = Tile::set();
        self.rng.shuffle(&mut set);
        let size = self.rules.hand_size(self.players);
        for hand in &mut self.hands {
            *hand = set.split_off(set.len() - size);
            hand.sort();
        }
        self.boneyard = set;
        self.board = Board::new(self.rules.spinner);
        self.leader = leader;
        self.turn = leader;
        self.passes = 0;
        self.must_play = None;
        self.voids = vec![[false; TOP as usize + 1]; self.players];
        self.phase = Phase::Playing;
        self.hand_no += 1;
        vec![Event::Dealt {
            who: leader,
            by_lot,
        }]
    }

    pub fn target(&self) -> u32 {
        self.rules.target(self.players, self.teams)
    }

    pub fn side(&self, seat: usize) -> usize {
        if self.teams { seat % 2 } else { seat }
    }

    pub fn sides(&self) -> usize {
        self.scores.len()
    }

    pub fn legal(&self) -> Vec<Action> {
        if !matches!(self.phase, Phase::Playing) {
            return Vec::new();
        }
        legal(
            &self.rules,
            &self.board,
            &self.hands[self.turn],
            self.boneyard.len(),
            self.must_play,
        )
    }

    /// The current player's action. Anything not legal is refused, which
    /// matters once actions come from a network.
    pub fn apply(&mut self, action: Action) -> Result<Vec<Event>, String> {
        if !self.legal().contains(&action) {
            return Err(format!("{action:?} is not a legal move"));
        }
        let who = self.turn;
        let mut events = Vec::new();
        match action {
            Action::Play(tile, arm) => {
                let hand = &mut self.hands[who];
                hand.retain(|&t| t != tile);
                self.board.play(tile, arm);
                self.passes = 0;
                self.must_play = None;
                let scored = if self.rules.score_ends {
                    self.board.score()
                } else {
                    0
                };
                let side = self.side(who);
                self.scores[side] += scored;
                events.push(Event::Played {
                    who,
                    tile,
                    arm,
                    scored,
                });
                if scored > 0 && self.scores[side] >= self.target() {
                    // All Fives: the game ends the moment the target is reached
                    self.phase = Phase::GameOver { winner: side };
                    events.push(Event::GameOver { winner: side });
                } else if self.hands[who].is_empty() {
                    events.extend(self.end_hand(Some(who)));
                } else {
                    self.turn = (who + 1) % self.players;
                }
            }
            Action::Draw => {
                if self.rules.drawing == Drawing::Forced {
                    self.note_void(who);
                }
                let tile = self.boneyard.pop().expect("a tile to draw");
                self.hands[who].push(tile);
                self.hands[who].sort();
                if self.rules.drawing == Drawing::Forced && self.board.fits(tile) {
                    self.must_play = Some(tile);
                }
                events.push(Event::Drew {
                    who,
                    tile: Some(tile),
                });
            }
            Action::Pass => {
                self.note_void(who);
                self.passes += 1;
                events.push(Event::Passed { who });
                if self.passes >= self.players {
                    events.extend(self.end_hand(None));
                } else {
                    self.turn = (who + 1) % self.players;
                }
            }
        }
        Ok(events)
    }

    fn note_void(&mut self, who: usize) {
        for (_, n) in self.board.ends() {
            self.voids[who][n as usize] = true;
        }
    }

    fn end_hand(&mut self, dominoed: Option<usize>) -> Vec<Event> {
        let sides = self.sides();
        let pips: Vec<u32> = self.hands.iter().map(|h| tile::pips(h)).collect();
        let mut held = vec![0; sides];
        for (seat, p) in pips.iter().enumerate() {
            held[self.side(seat)] += p;
        }
        let total: u32 = pips.iter().sum();
        let low = *held.iter().min().expect("sides");
        let lowest: Vec<usize> = (0..sides).filter(|&s| held[s] == low).collect();
        let mut points = vec![0; sides];
        let winner = match dominoed {
            Some(w) => Some(self.side(w)),
            None if lowest.len() == 1 => Some(lowest[0]),
            None => None,
        };
        match (self.rules.hand_score, winner) {
            // partners: the losing side's pips, the winners' own ignored
            // (pagat, Partnership Dominoes)
            (HandScore::LowestTakesDifference, Some(w)) if self.teams => {
                points[w] = total - held[w];
            }
            (HandScore::LowestTakesDifference, Some(w)) => {
                points[w] = (total - held[w]).saturating_sub(held[w]);
            }
            (HandScore::OpponentsRounded, Some(w)) => {
                points[w] = round_to_five(total - held[w]);
            }
            // All Fives, three players: two tied for lowest split the
            // third's count
            (HandScore::OpponentsRounded, None) if sides == 3 && lowest.len() == 2 => {
                let third = (0..sides).find(|i| !lowest.contains(i)).expect("a third");
                for &i in &lowest {
                    points[i] = round_to_five(held[third]) / 2;
                }
            }
            _ => {}
        }
        for (score, p) in self.scores.iter_mut().zip(&points) {
            *score += p;
        }
        let result = HandResult {
            hands: self.hands.clone(),
            pips,
            dominoed,
            winner,
            points,
        };
        let mut events = vec![Event::HandOver(result.clone())];
        let best = *self.scores.iter().max().expect("sides");
        let leaders: Vec<usize> = (0..sides).filter(|&i| self.scores[i] == best).collect();
        if best >= self.target() && leaders.len() == 1 {
            self.phase = Phase::GameOver { winner: leaders[0] };
            events.push(Event::GameOver { winner: leaders[0] });
        } else {
            self.phase = Phase::HandOver(result);
        }
        events
    }

    /// Deal the next hand, after a `HandOver`.
    pub fn next_hand(&mut self) -> Vec<Event> {
        let Phase::HandOver(result) = &self.phase else {
            return Vec::new();
        };
        let (leader, by_lot) = match (self.rules.next_lead, result.dominoed) {
            (NextLead::Rotate, _) => ((self.leader + 1) % self.players, false),
            (NextLead::Winner, Some(w)) => (w, false),
            (NextLead::Winner, None) => (self.rng.below(self.players), true),
        };
        self.deal(leader, by_lot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{ALL, ALL_FIVES, BLOCK, DRAW};

    /// Play a whole game with the first legal action each turn; check the
    /// tiles are all accounted for after every action.
    fn play_out(rules: Rules, players: usize, teams: bool, seed: u64) -> Game {
        let (mut g, _) = Game::new(rules, players, teams, seed);
        for _ in 0..10_000 {
            match g.phase {
                Phase::Playing => {
                    let a = g.legal()[0];
                    g.apply(a).unwrap();
                    let count = g.board.len()
                        + g.boneyard.len()
                        + g.hands.iter().map(Vec::len).sum::<usize>();
                    assert_eq!(count, 28);
                }
                Phase::HandOver(_) => {
                    g.next_hand();
                }
                Phase::GameOver { .. } => return g,
            }
        }
        panic!("the game did not end");
    }

    #[test]
    fn every_rule_set_plays_to_the_end() {
        for rules in ALL {
            for (players, teams) in [(2, false), (3, false), (4, false), (4, true)] {
                for seed in 0..20 {
                    let g = play_out(rules, players, teams, seed);
                    let Phase::GameOver { winner } = g.phase else {
                        unreachable!()
                    };
                    assert!(g.scores[winner] >= g.target());
                }
            }
        }
    }

    #[test]
    fn deals_follow_the_rules() {
        let (g, _) = Game::new(BLOCK, 3, false, 1);
        assert!(g.hands.iter().all(|h| h.len() == 5));
        let (g, _) = Game::new(DRAW, 4, false, 1);
        assert!(g.hands.iter().all(|h| h.len() == 6));
        assert_eq!(g.boneyard.len(), 4);
        let (g, _) = Game::new(ALL_FIVES, 2, false, 1);
        assert!(g.hands.iter().all(|h| h.len() == 9));
    }

    #[test]
    fn the_draw_game_leaves_two_in_the_boneyard() {
        let (mut g, _) = Game::new(DRAW, 2, false, 7);
        let lead = g.legal()[0];
        g.apply(lead).unwrap();
        while g.legal().contains(&Action::Draw) {
            g.apply(Action::Draw).unwrap();
        }
        assert_eq!(g.boneyard.len(), 2);
    }

    #[test]
    fn a_drawn_tile_that_fits_must_be_played() {
        for seed in 0..50 {
            let (mut g, _) = Game::new(ALL_FIVES, 4, false, seed);
            let lead = g.legal()[0];
            g.apply(lead).unwrap();
            while g.legal() != vec![Action::Draw] {
                let a = g.legal()[0];
                g.apply(a).unwrap();
                if !matches!(g.phase, Phase::Playing) {
                    break;
                }
            }
            if !matches!(g.phase, Phase::Playing) {
                continue;
            }
            loop {
                g.apply(Action::Draw).unwrap();
                if let Some(m) = g.must_play {
                    assert!(
                        g.legal()
                            .iter()
                            .all(|a| matches!(a, Action::Play(t, _) if *t == m))
                    );
                    return;
                }
                if g.legal() != vec![Action::Draw] {
                    break;
                }
            }
        }
        panic!("no seed drew a playable tile");
    }

    #[test]
    fn illegal_actions_are_refused() {
        let (mut g, _) = Game::new(BLOCK, 2, false, 3);
        assert!(g.apply(Action::Pass).is_err());
        assert!(g.apply(Action::Draw).is_err());
        let other = g.hands[(g.turn + 1) % 2][0];
        assert!(g.apply(Action::Play(other, Arm::Left)).is_err());
    }

    fn hand_over(
        rules: Rules,
        teams: bool,
        hands: Vec<Vec<Tile>>,
        dominoed: Option<usize>,
    ) -> HandResult {
        let (mut g, _) = Game::new(rules, hands.len(), teams, 0);
        g.hands = hands;
        g.end_hand(dominoed);
        match g.phase {
            Phase::HandOver(r) => r,
            Phase::GameOver { .. } | Phase::Playing => unreachable!(),
        }
    }

    #[test]
    fn block_scoring_is_the_losers_less_the_winner() {
        let t = Tile::new;
        let r = hand_over(
            BLOCK,
            false,
            vec![vec![t(0, 3)], vec![t(6, 6)], vec![t(4, 5)]],
            None,
        );
        assert_eq!(r.winner, Some(0));
        assert_eq!(r.points, vec![12 + 9 - 3, 0, 0]);
        let r = hand_over(BLOCK, false, vec![vec![t(1, 2)], vec![t(0, 3)]], None);
        assert_eq!(r.winner, None);
        assert_eq!(r.points, vec![0, 0]);
    }

    #[test]
    fn all_fives_scoring_rounds_the_opponents() {
        let t = Tile::new;
        let r = hand_over(
            ALL_FIVES,
            false,
            vec![vec![], vec![t(6, 6)], vec![t(1, 0)]],
            Some(0),
        );
        assert_eq!(r.points, vec![15, 0, 0]);
        // blocked, three players, two tied: they split the third's count
        let r = hand_over(
            ALL_FIVES,
            false,
            vec![vec![t(1, 1)], vec![t(0, 2)], vec![t(6, 4)]],
            None,
        );
        assert_eq!(r.points, vec![5, 5, 0]);
    }

    #[test]
    fn partners_pool_their_pips() {
        let t = Tile::new;
        // seats 0 and 2 hold 3 + 4, seats 1 and 3 hold 1 + 12: the
        // single lowest hand is seat 1's, but side 0 holds fewer
        let hands = vec![vec![t(0, 3)], vec![t(0, 1)], vec![t(1, 3)], vec![t(6, 6)]];
        let r = hand_over(BLOCK, true, hands.clone(), None);
        assert_eq!(r.winner, Some(0));
        assert_eq!(r.points, vec![13, 0]);
        let r = hand_over(ALL_FIVES, true, hands, None);
        assert_eq!(r.points, vec![15, 0]);
    }

    #[test]
    fn partners_score_together_in_play() {
        let (mut g, _) = Game::new(ALL_FIVES, 4, true, 2);
        assert_eq!(g.scores.len(), 2);
        while matches!(g.phase, Phase::Playing) {
            let a = g.legal()[0];
            let side = g.side(g.turn);
            let before = g.scores[side];
            let events = g.apply(a).unwrap();
            if let [Event::Played { scored, .. }] = events[..] {
                assert_eq!(g.scores[side], before + scored);
            }
        }
    }
}
