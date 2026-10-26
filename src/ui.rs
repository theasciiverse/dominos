//! Drawing the screens: the menu, and the table with the layout in the
//! middle, the others' hands above, the person's hand below.

use crate::app::{App, Hits, Overlay, Screen};
use crate::draw::{
    self, BONE_LAST, CRIMSON, FELT, FELT_DARK, GOLD, IVORY, IVORY_DIM, Look, MUTED, TEXT, backs,
    centred, fill, put,
};
use crate::game::{Action, HandResult, Phase};
use crate::help;
use crate::layout::{self, Placed};
use crate::rules::{self, Drawing, Kind};
use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

const MIN: (i32, i32) = (64, 22);

pub fn render(app: &App, buf: &mut Buffer) -> Hits {
    let (w, h) = (buf.area.width as i32, buf.area.height as i32);
    fill(buf, 0, 0, w, h, FELT);
    let mut hits = Hits::default();
    if w < MIN.0 || h < MIN.1 {
        let s = Style::new().fg(TEXT).bg(FELT);
        centred(
            buf,
            0,
            h / 2,
            w,
            &format!("Make the window at least {}x{}", MIN.0, MIN.1),
            s,
        );
        return hits;
    }
    match app.screen {
        Screen::Menu => menu(app, buf, w, h, &mut hits),
        Screen::Table => table(app, buf, w, h, &mut hits),
    }
    hits
}

/// The name in letters cut from blocks.
const WORDMARK: [&str; 3] = [
    "█▀▀▄ ▄▀▀▄ █▄ ▄█ ▀█▀ █▄  █ ▄▀▀▄ ▄▀▀▀",
    "█  █ █  █ █ ▀ █  █  █ ▀▄█ █  █  ▀▀▄",
    "█▄▄▀ ▀▄▄▀ █   █ ▄█▄ █   █ ▀▄▄▀ ▄▄▄▀",
];

fn menu(app: &App, buf: &mut Buffer, w: i32, h: i32, hits: &mut Hits) {
    let top = ((h - 24) / 2).max(0);
    let mark = Style::new().fg(IVORY).bg(FELT);
    for (i, row) in WORDMARK.iter().enumerate() {
        centred(buf, 0, top + 1 + i as i32, w, row, mark);
    }
    // a few tiles under the name, as if just played
    let x = w / 2 - 19;
    draw::large(buf, x, top + 6, false, 5, 3, Look::Normal);
    draw::large(buf, x + 12, top + 4, true, 3, 3, Look::Normal);
    draw::large(buf, x + 18, top + 6, false, 3, 6, Look::Last);
    draw::large(buf, x + 30, top + 6, false, 6, 1, Look::Normal);

    let setup = &app.setup;
    let rules = setup.rules();
    let rows = [
        ("Game", rules.name.to_string()),
        ("Opponents", format!("{}", setup.opponents)),
        ("Computer", setup.strength.name().to_string()),
    ];
    let y0 = top + 12;
    let label_x = w / 2 - 20;
    for (i, (label, value)) in rows.iter().enumerate() {
        let y = y0 + i as i32 * 2 + if i > 0 { 2 } else { 0 };
        let on = app.menu_row == i;
        let style = if on {
            Style::new().fg(IVORY).bg(FELT).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(MUTED).bg(FELT)
        };
        put(buf, label_x, y, label, style);
        let value = if on {
            format!("◂ {value} ▸")
        } else {
            format!("  {value}  ")
        };
        let vstyle = if on {
            Style::new()
                .fg(IVORY)
                .bg(CRIMSON)
                .add_modifier(Modifier::BOLD)
        } else {
            style
        };
        put(buf, label_x + 13, y, &value, vstyle);
        hits.menu.push((label_x, y, 40, 1, i));
        if i == 0 {
            let small = Style::new().fg(MUTED).bg(FELT);
            put(
                buf,
                label_x + 2,
                y + 1,
                rules.blurb,
                Style::new().fg(TEXT).bg(FELT),
            );
            put(
                buf,
                label_x + 2,
                y + 2,
                &format!("Rules: {}", rules.source),
                small.add_modifier(Modifier::ITALIC),
            );
        }
    }
    let y = y0 + 9;
    let on = app.menu_row == 3;
    let start = if on { "▸  Deal  ◂" } else { "   Deal   " };
    let style = if on {
        Style::new()
            .fg(IVORY)
            .bg(CRIMSON)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(TEXT).bg(FELT_DARK)
    };
    centred(buf, 0, y, w, start, style);
    hits.menu.push((w / 2 - 6, y, 12, 1, 3));
    let keys = "↑↓ choose   ←→ change   Enter deal   Q quit";
    centred(buf, 0, h - 1, w, keys, Style::new().fg(MUTED).bg(FELT));
}

fn table(app: &App, buf: &mut Buffer, w: i32, h: i32, hits: &mut Hits) {
    let game = &app.game;
    let band = Style::new().fg(TEXT).bg(FELT_DARK);
    let muted_band = Style::new().fg(MUTED).bg(FELT_DARK);

    // header: name, game, the boneyard
    fill(buf, 0, 0, w, 1, FELT_DARK);
    put(
        buf,
        0,
        0,
        " DOMINOS ",
        Style::new()
            .fg(IVORY)
            .bg(CRIMSON)
            .add_modifier(Modifier::BOLD),
    );
    let what = format!(
        " {} · to {} · hand {}",
        game.rules.name,
        game.target(),
        game.hand_no
    );
    put(buf, 9, 0, &what, band);
    if game.rules.drawing != Drawing::None {
        let n = game.boneyard.len();
        let text = format!("boneyard {} {n} ", backs(n.min(14)));
        let x = w - text.chars().count() as i32;
        put(buf, x, 0, &text, band);
        hits.boneyard = Some((x, 0, w - x, 1));
    }

    // the others, each with their tiles face down and their score
    fill(buf, 0, 1, w, 1, FELT_DARK);
    let others = app.players() - 1;
    let cell = w / others as i32;
    for seat in 1..app.players() {
        let x = (seat as i32 - 1) * cell + 1;
        let turn = game.turn == seat && matches!(game.phase, Phase::Playing);
        let marker = if turn { "▸ " } else { "  " };
        put(
            buf,
            x,
            1,
            marker,
            Style::new()
                .fg(CRIMSON)
                .bg(FELT_DARK)
                .add_modifier(Modifier::BOLD),
        );
        let name_style = if turn {
            band.add_modifier(Modifier::BOLD)
        } else {
            band
        };
        put(buf, x + 2, 1, app.name(seat), name_style);
        let n = game.hands[seat].len();
        put(
            buf,
            x + 9,
            1,
            &backs(n),
            Style::new().fg(IVORY_DIM).bg(FELT_DARK),
        );
        let score = format!("{:>3}", game.scores[seat]);
        put(
            buf,
            x + 9 + n as i32 + 1,
            1,
            &score,
            Style::new().fg(GOLD).bg(FELT_DARK),
        );
    }

    // the person's hand: large tiles standing, or small ones when many
    let hand = &game.hands[0];
    let large = hand.len() as i32 * 6 + 2 <= w;
    let hand_h = if large { 9 } else { 5 };
    let hand_y = h - 1 - hand_h;
    let board_y = 3;
    let board_h = hand_y - 3 - board_y;

    // the layout
    let (ghosts, chosen) = app.ghosts();
    let lay = layout::layout(&game.board, &ghosts, w - 2, board_h);
    for p in &lay.tiles {
        let look = match p.ghost {
            Some(arm) if Some(arm) == chosen => Look::Chosen,
            Some(_) => Look::Ghost,
            None if Some(p.tile()) == app.last => Look::Last,
            None => Look::Normal,
        };
        tile(buf, p, 1, board_y, lay.size.is_large(), look);
        if let Some(arm) = p.ghost {
            hits.ends.push((p.x + 1, p.y + board_y, p.w, p.h, arm));
        }
    }
    if game.rules.kind == Kind::AllFives && !game.board.is_empty() {
        let count = game.board.count();
        let five = count.is_multiple_of(5);
        let text = format!(" ends {count} ");
        let style = if five {
            Style::new()
                .fg(FELT_DARK)
                .bg(BONE_LAST)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(MUTED).bg(FELT)
        };
        put(buf, w - text.chars().count() as i32 - 1, 2, &text, style);
    }

    // the last things said
    let log_y = hand_y - 2;
    for (i, line) in app.log.iter().rev().take(2).enumerate() {
        let style = if i == 0 {
            Style::new().fg(TEXT).bg(FELT)
        } else {
            Style::new().fg(MUTED).bg(FELT)
        };
        put(buf, 2, log_y + 1 - i as i32, line, style);
    }

    // the hand
    let mine = app.your_turn();
    let legal = if mine { game.legal() } else { Vec::new() };
    let playable = |t| {
        legal
            .iter()
            .any(|a| matches!(a, Action::Play(p, _) if *p == t))
    };
    let step = if large { 6 } else { 4 };
    let x0 = (w - hand.len() as i32 * step + 1) / 2;
    for (i, &t) in hand.iter().enumerate() {
        let x = x0 + i as i32 * step;
        let picked = i == app.cursor && mine;
        let y = hand_y + if picked { 0 } else { 1 };
        let look = if mine && !playable(t) {
            Look::Dim
        } else {
            Look::Normal
        };
        if large {
            draw::large(buf, x, y, true, t.hi, t.lo, look);
        } else {
            draw::small(buf, x, y, true, t.hi, t.lo, look);
        }
        if picked && mine {
            let bar = if large {
                "▀▀▀▀▀"
            } else {
                "▀▀▀"
            };
            let under = y + if large { 7 } else { 3 };
            put(buf, x, under, bar, Style::new().fg(CRIMSON).bg(FELT));
        }
        hits.hand.push((x, hand_y, step - 1, hand_h, i));
    }

    // footer: the person's score and what to do
    let you_turn = game.turn == 0 && matches!(game.phase, Phase::Playing);
    fill(buf, 0, h - 1, w, 1, FELT_DARK);
    let marker = if you_turn { "▸ " } else { "  " };
    put(
        buf,
        1,
        h - 1,
        marker,
        Style::new()
            .fg(CRIMSON)
            .bg(FELT_DARK)
            .add_modifier(Modifier::BOLD),
    );
    put(
        buf,
        3,
        h - 1,
        app.name(0),
        band.add_modifier(Modifier::BOLD),
    );
    put(
        buf,
        9,
        h - 1,
        &format!("{:>3}", game.scores[0]),
        Style::new().fg(GOLD).bg(FELT_DARK),
    );
    let hint = if app.demo {
        "demo · Q quit".to_string()
    } else if !mine {
        "? help · Q menu".to_string()
    } else if app.choosing.is_some() {
        "←→ choose the end · Enter play · Esc back".to_string()
    } else if legal == vec![Action::Draw] {
        "You cannot play: D draw".to_string()
    } else {
        let draw = if legal.contains(&Action::Draw) {
            " · D draw"
        } else {
            ""
        };
        format!("←→ tile · Enter play{draw} · ? help · Q menu")
    };
    let x = w - hint.chars().count() as i32 - 1;
    put(buf, x, h - 1, &hint, muted_band);

    match &app.overlay {
        Overlay::None => {}
        Overlay::Help => help_box(app, buf, w, h),
        Overlay::HandOver(result) => hand_over(app, buf, w, h, result),
        Overlay::GameOver => {
            if let Phase::GameOver { winner } = app.game.phase {
                game_over(app, buf, w, h, winner);
            }
        }
    }
}

fn tile(buf: &mut Buffer, p: &Placed, dx: i32, dy: i32, large: bool, look: Look) {
    let (x, y) = (p.x + dx, p.y + dy);
    if large {
        draw::large(buf, x, y, p.upright, p.first, p.second, look);
    } else {
        draw::small(buf, x, y, p.upright, p.first, p.second, look);
    }
}

/// A framed box in the middle of the screen; returns its inside.
fn frame(buf: &mut Buffer, w: i32, h: i32, bw: i32, bh: i32, title: &str) -> (i32, i32) {
    let (x, y) = ((w - bw) / 2, (h - bh) / 2);
    fill(buf, x, y, bw, bh, FELT_DARK);
    let edge = Style::new().fg(GOLD).bg(FELT_DARK);
    let line = "─".repeat(bw as usize - 2);
    put(buf, x, y, &format!("╭{line}╮"), edge);
    put(buf, x, y + bh - 1, &format!("╰{line}╯"), edge);
    for r in 1..bh - 1 {
        put(buf, x, y + r, "│", edge);
        put(buf, x + bw - 1, y + r, "│", edge);
    }
    let t = format!(" {title} ");
    centred(
        buf,
        x,
        y,
        bw,
        &t,
        Style::new()
            .fg(IVORY)
            .bg(CRIMSON)
            .add_modifier(Modifier::BOLD),
    );
    (x + 2, y + 1)
}

fn help_box(app: &App, buf: &mut Buffer, w: i32, h: i32) {
    let lines: Vec<&str> = help::rules(app.game.rules.kind)
        .iter()
        .chain([""].iter())
        .chain(help::KEYS.iter())
        .copied()
        .collect();
    let bw = 74.min(w);
    let bh = (lines.len() as i32 + 4).min(h);
    let (x, y) = frame(buf, w, h, bw, bh, "Help");
    for (i, line) in lines.iter().enumerate() {
        let heading = line.chars().next().is_some_and(|c| c.is_uppercase())
            && line.chars().take(4).all(|c| c.is_uppercase() || c == ' ');
        let style = if heading {
            Style::new()
                .fg(GOLD)
                .bg(FELT_DARK)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(TEXT).bg(FELT_DARK)
        };
        put(buf, x, y + 1 + i as i32, line, style);
    }
}

fn hand_over(app: &App, buf: &mut Buffer, w: i32, h: i32, r: &HandResult) {
    let n = app.players();
    let bw = 66.min(w);
    let bh = n as i32 * 2 + 8;
    let title = match (r.dominoed, r.winner) {
        (Some(p), _) => format!("{} played out", app.name(p)),
        (None, Some(p)) => format!("Blocked · {} has the fewest pips", app.name(p)),
        (None, None) => "Blocked · tied for the fewest".to_string(),
    };
    let (x, y) = frame(buf, w, h, bw, bh, &title);
    let head = Style::new().fg(MUTED).bg(FELT_DARK);
    put(buf, x, y + 1, "", head);
    put(buf, x + 40, y + 1, "pips   hand   game", head);
    for seat in 0..n {
        let row = y + 2 + seat as i32 * 2;
        let style = Style::new().fg(TEXT).bg(FELT_DARK);
        let bold = if r.winner == Some(seat) {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        };
        put(buf, x, row, app.name(seat), bold);
        let mut tx = x + 7;
        for &t in r.hands[seat].iter().take(5) {
            draw::small(buf, tx, row, false, t.hi, t.lo, Look::Normal);
            tx += 6;
        }
        if r.hands[seat].len() > 5 {
            put(buf, tx, row, "…", style);
        }
        put(buf, x + 40, row, &format!("{:>4}", r.pips[seat]), style);
        let pts = if r.points[seat] > 0 {
            format!("+{}", r.points[seat])
        } else {
            "·".into()
        };
        let pstyle = if r.points[seat] > 0 {
            Style::new()
                .fg(GOLD)
                .bg(FELT_DARK)
                .add_modifier(Modifier::BOLD)
        } else {
            style
        };
        put(buf, x + 47, row, &format!("{pts:>4}"), pstyle);
        put(
            buf,
            x + 54,
            row,
            &format!("{:>4}", app.game.scores[seat]),
            bold,
        );
    }
    let note = match app.game.rules.hand_score {
        rules::HandScore::LowestTakesDifference => {
            "The lowest count scores the others' pips less its own."
        }
        rules::HandScore::OpponentsRounded => {
            "The winner scores the others' pips, to the nearest five."
        }
    };
    put(
        buf,
        x,
        y + bh - 4,
        note,
        Style::new().fg(MUTED).bg(FELT_DARK),
    );
    let next = if app.demo { "" } else { "Enter: next hand" };
    centred(
        buf,
        x - 2,
        y + bh - 3,
        bw,
        next,
        Style::new()
            .fg(IVORY)
            .bg(FELT_DARK)
            .add_modifier(Modifier::BOLD),
    );
}

fn game_over(app: &App, buf: &mut Buffer, w: i32, h: i32, winner: usize) {
    let n = app.players();
    let bw = 44.min(w);
    let bh = n as i32 + 7;
    let title = if winner == 0 && !app.demo {
        "You win the game!".to_string()
    } else {
        format!("{} wins the game", app.name(winner))
    };
    let (x, y) = frame(buf, w, h, bw, bh, &title);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&s| std::cmp::Reverse(app.game.scores[s]));
    for (i, &seat) in order.iter().enumerate() {
        let style = if seat == winner {
            Style::new()
                .fg(GOLD)
                .bg(FELT_DARK)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(TEXT).bg(FELT_DARK)
        };
        put(buf, x + 8, y + 2 + i as i32, app.name(seat), style);
        put(
            buf,
            x + 24,
            y + 2 + i as i32,
            &format!("{:>4}", app.game.scores[seat]),
            style,
        );
    }
    if !app.demo {
        let s = Style::new()
            .fg(IVORY)
            .bg(FELT_DARK)
            .add_modifier(Modifier::BOLD);
        centred(buf, x - 2, y + bh - 3, bw, "Enter: play again · Q: menu", s);
    }
}
