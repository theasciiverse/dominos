//! The help shown over the table: the rules of the game being played, in
//! short, and the keys.

use crate::rules::Kind;

pub fn rules(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Block => &[
            "BLOCK GAME  (after pagat.com, Block Dominoes)",
            "",
            "Seven tiles each for two players, five for three or four; the rest",
            "stay face down and out of play. The leader plays any tile; then each",
            "in turn lays a tile matching an open end. Doubles lie across the line",
            "but are not spinners. If you cannot play, you pass.",
            "",
            "The hand ends when someone plays their last tile, or nobody can play.",
            "The lowest count of pips left wins the hand and scores the losers'",
            "pips less their own; a tie for lowest scores nothing. The lead passes",
            "to the left. Game: 100 for two players, 61 for three or four.",
        ],
        Kind::Draw => &[
            "DRAW GAME  (after pagat.com, Draw Dominoes)",
            "",
            "Seven tiles each for two or three players, six for four; the rest",
            "make the boneyard. Play as in the Block game, but you may draw from",
            "the boneyard whenever you like, even holding a play, until only two",
            "tiles are left there; those two are never drawn. If you cannot play",
            "and cannot draw, you pass.",
            "",
            "Scoring as in the Block game: the lowest count wins the losers' pips",
            "less their own, a tie scores nothing, the lead passes to the left.",
            "Game: 100 for two players, 61 for three or four.",
        ],
        Kind::AllFives => &[
            "ALL FIVES  (after pagat.com, All Fives)",
            "",
            "Nine tiles each for two players, seven for three, five for four. The",
            "first double is the spinner: once both its ends are covered, tiles",
            "may go on its two long sides too, making four arms. You must play if",
            "you can; if not, draw until you can (a tile drawn that fits is played",
            "at once) or the boneyard is empty, then pass.",
            "",
            "After each play, add up the open ends: a double at an end counts both",
            "halves, the spinner counts until both its ends are covered. A multiple",
            "of five scores at once. Whoever plays out, or holds the fewest pips",
            "when nobody can play, scores the others' pips to the nearest five.",
            "First to 250 (two players) or 200 wins, the moment it is reached.",
        ],
    }
}

pub const KEYS: &[&str] = &[
    "KEYS",
    "",
    "← →  or  h l     choose a tile from your hand (or click it)",
    "Enter  Space      play it; where it fits two ways, ← → choose the end",
    "D                 draw from the boneyard",
    "?  F1             this help          Q  Esc   back to the menu",
];
