//! Colours and the drawing of tiles: a green baize table, ivory tiles,
//! black pips, crimson for what is chosen.

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};

pub const FELT: Color = Color::Rgb(18, 74, 50);
pub const FELT_DARK: Color = Color::Rgb(11, 50, 33);
pub const FELT_LIGHT: Color = Color::Rgb(44, 104, 76);
pub const IVORY: Color = Color::Rgb(238, 230, 208);
pub const IVORY_DIM: Color = Color::Rgb(160, 156, 140);
pub const BONE_LAST: Color = Color::Rgb(250, 236, 160);
pub const PIP: Color = Color::Rgb(26, 24, 28);
pub const PIP_DIM: Color = Color::Rgb(96, 92, 86);
pub const SEAM: Color = Color::Rgb(150, 126, 96);
pub const CRIMSON: Color = Color::Rgb(200, 40, 54);
pub const TEXT: Color = Color::Rgb(236, 228, 205);
pub const MUTED: Color = Color::Rgb(138, 178, 152);
pub const GOLD: Color = Color::Rgb(232, 190, 90);

/// How a tile is shown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    Normal,
    /// In your hand but no play now.
    Dim,
    /// The tile just played.
    Last,
    /// Where a tile could go.
    Ghost,
    /// Where the chosen tile will go.
    Chosen,
}

impl Look {
    fn colours(self) -> (Color, Color, Color) {
        // face, pips, seam
        match self {
            Look::Normal => (IVORY, PIP, SEAM),
            Look::Dim => (IVORY_DIM, PIP_DIM, PIP_DIM),
            Look::Last => (BONE_LAST, PIP, SEAM),
            Look::Ghost => (FELT_LIGHT, IVORY_DIM, MUTED),
            Look::Chosen => (CRIMSON, IVORY, IVORY_DIM),
        }
    }
}

pub fn put(buf: &mut Buffer, x: i32, y: i32, s: &str, style: Style) {
    let area = buf.area;
    if y < area.y as i32 || y >= (area.y + area.height) as i32 {
        return;
    }
    for (i, ch) in s.chars().enumerate() {
        let cx = x + i as i32;
        if cx >= area.x as i32 && cx < (area.x + area.width) as i32 {
            buf[(cx as u16, y as u16)].set_char(ch).set_style(style);
        }
    }
}

pub fn fill(buf: &mut Buffer, x: i32, y: i32, w: i32, h: i32, bg: Color) {
    let line = " ".repeat(w.max(0) as usize);
    for row in y..y + h {
        put(buf, x, row, &line, Style::new().bg(bg));
    }
}

/// Text centred on `width` cells from `x`.
pub fn centred(buf: &mut Buffer, x: i32, y: i32, width: i32, s: &str, style: Style) {
    let len = s.chars().count() as i32;
    put(buf, x + (width - len) / 2, y, s, style);
}

/// The 3x3 pip grid for 0 to 6: (row, column).
fn pip_spots(n: u8) -> &'static [(i32, i32)] {
    match n {
        1 => &[(1, 1)],
        2 => &[(0, 0), (2, 2)],
        3 => &[(0, 0), (1, 1), (2, 2)],
        4 => &[(0, 0), (0, 2), (2, 0), (2, 2)],
        5 => &[(0, 0), (0, 2), (1, 1), (2, 0), (2, 2)],
        6 => &[(0, 0), (0, 2), (1, 0), (1, 2), (2, 0), (2, 2)],
        _ => &[],
    }
}

/// One half of a large tile, 5 by 3, its pips in columns 1 to 3.
fn half(buf: &mut Buffer, x: i32, y: i32, n: u8, face: Color, pip: Color) {
    fill(buf, x, y, 5, 3, face);
    for &(r, c) in pip_spots(n) {
        put(buf, x + 1 + c, y + r, "●", Style::new().fg(pip).bg(face));
    }
}

/// A large tile: 11x3 lying across, 5x7 standing up.
pub fn large(buf: &mut Buffer, x: i32, y: i32, upright: bool, a: u8, b: u8, look: Look) {
    let (face, pip, seam) = look.colours();
    let seam_style = Style::new().fg(seam).bg(face);
    if upright {
        half(buf, x, y, a, face, pip);
        put(buf, x, y + 3, "─────", seam_style);
        half(buf, x, y + 4, b, face, pip);
    } else {
        half(buf, x, y, a, face, pip);
        for r in 0..3 {
            put(buf, x + 5, y + r, "│", seam_style);
        }
        half(buf, x + 6, y, b, face, pip);
    }
}

/// A small tile with numbers: 5x1 lying across, 3x3 standing up.
pub fn small(buf: &mut Buffer, x: i32, y: i32, upright: bool, a: u8, b: u8, look: Look) {
    let (face, pip, seam) = look.colours();
    let num = Style::new().fg(pip).bg(face).add_modifier(Modifier::BOLD);
    let seam_style = Style::new().fg(seam).bg(face);
    if upright {
        put(buf, x, y, &format!(" {a} "), num);
        put(buf, x, y + 1, "───", seam_style);
        put(buf, x, y + 2, &format!(" {b} "), num);
    } else {
        put(buf, x, y, &format!(" {a}"), num);
        put(buf, x + 2, y, "│", seam_style);
        put(buf, x + 3, y, &format!("{b} "), num);
    }
}

/// The backs of `n` tiles, as a row of small bars.
pub fn backs(n: usize) -> String {
    "▮".repeat(n)
}

/// A tile written out, for messages: 6|4.
pub fn named(t: crate::tile::Tile) -> String {
    format!("{}|{}", t.hi, t.lo)
}
