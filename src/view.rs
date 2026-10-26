//! What one seat can see of the game: its own hand, the table, how many
//! tiles the others hold, the scores. The screen is drawn from it, the
//! computer players choose from it, and over a network it is what the
//! host sends each player.

use crate::board::Board;
use crate::game::{Action, Game, Phase, legal};
use crate::rules::{self, Kind, Rules};
use crate::tile::{TOP, Tile};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct View {
    /// Whose view this is.
    pub seat: usize,
    pub kind: Kind,
    pub players: usize,
    pub teams: bool,
    pub hand: Vec<Tile>,
    /// How many tiles each seat holds.
    pub counts: Vec<usize>,
    pub board: Board,
    pub boneyard: usize,
    /// By side.
    pub scores: Vec<u32>,
    pub target: u32,
    pub turn: usize,
    pub hand_no: u32,
    /// A tile this seat drew and must play at once.
    pub must_play: Option<Tile>,
    pub voids: Vec<[bool; TOP as usize + 1]>,
    pub phase: Phase,
    /// The people at the table by seat, with their names; `None` for a
    /// computer player.
    pub names: Vec<Option<String>>,
    /// The seats the next deal waits for.
    pub waiting: Vec<usize>,
}

impl Game {
    pub fn view(&self, seat: usize) -> View {
        View {
            seat,
            kind: self.rules.kind,
            players: self.players,
            teams: self.teams,
            hand: self.hands[seat].clone(),
            counts: self.hands.iter().map(Vec::len).collect(),
            board: self.board.clone(),
            boneyard: self.boneyard.len(),
            scores: self.scores.clone(),
            target: self.target(),
            turn: self.turn,
            hand_no: self.hand_no,
            must_play: self.must_play.filter(|_| self.turn == seat),
            voids: self.voids.clone(),
            phase: self.phase.clone(),
            names: vec![None; self.players],
            waiting: Vec::new(),
        }
    }
}

impl View {
    pub fn rules(&self) -> Rules {
        rules::of(self.kind)
    }

    pub fn side(&self, seat: usize) -> usize {
        if self.teams { seat % 2 } else { seat }
    }

    pub fn playing(&self) -> bool {
        matches!(self.phase, Phase::Playing)
    }

    pub fn my_turn(&self) -> bool {
        self.playing() && self.turn == self.seat
    }

    /// What this seat may do now; nothing when it is not its turn.
    pub fn legal(&self) -> Vec<Action> {
        if !self.my_turn() {
            return Vec::new();
        }
        legal(
            &self.rules(),
            &self.board,
            &self.hand,
            self.boneyard,
            self.must_play,
        )
    }

    /// Where `seat` sits from this seat: 0 is here, 1 on the left, and on
    /// round.
    pub fn place(&self, seat: usize) -> usize {
        (seat + self.players - self.seat) % self.players
    }

    /// The tiles this seat cannot see: in other hands and the boneyard.
    pub fn unseen(&self) -> Vec<Tile> {
        let seen = self.board.tiles();
        Tile::set()
            .into_iter()
            .filter(|t| !seen.contains(t) && !self.hand.contains(t))
            .collect()
    }
}
