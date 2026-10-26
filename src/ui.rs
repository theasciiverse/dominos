//! Drawing the screens: the menu, finding and waiting for a network
//! table, and the table itself, with the layout in the middle, the
//! others' hands above and the person's hand below.

use crate::app::{App, Hits, ROW_HOST, ROW_JOIN, ROW_NAME, ROW_PLAY, Screen, Session};
use crate::draw::{
    self, BONE_LAST, CRIMSON, FELT, FELT_DARK, GOLD, IVORY, IVORY_DIM, Look, MUTED, TEXT, backs,
    centred, fill, put,
};
use crate::game::{Action, HandResult, Phase};
use crate::help;
use crate::layout::{self, Placed};
use crate::net::{self, Lobby};
use crate::rules::{self, Drawing, HandScore, Kind};
use crate::view::View;
use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

const MIN: (i32, i32) = (64, 22);

fn plain(fg: ratatui::style::Color) -> Style {
    Style::new().fg(fg).bg(FELT)
}

fn banded(fg: ratatui::style::Color) -> Style {
    Style::new().fg(fg).bg(FELT_DARK)
}

fn bold(style: Style) -> Style {
    style.add_modifier(Modifier::BOLD)
}

fn chosen() -> Style {
    bold(Style::new().fg(IVORY).bg(CRIMSON))
}

pub fn render(app: &App, buf: &mut Buffer) -> Hits {
    let (w, h) = (buf.area.width as i32, buf.area.height as i32);
    fill(buf, 0, 0, w, h, FELT);
    let mut hits = Hits::default();
    if w < MIN.0 || h < MIN.1 {
        let text = format!("Make the window at least {}x{}", MIN.0, MIN.1);
        centred(buf, 0, h / 2, w, &text, plain(TEXT));
        return hits;
    }
    match app.screen {
        Screen::Menu => menu(app, buf, w, h, &mut hits),
        Screen::Join => join(app, buf, w, h, &mut hits),
        Screen::Lobby => lobby(app, buf, w, h),
        Screen::Table => {
            if let Some(view) = app.view() {
                table(app, &view, buf, w, h, &mut hits);
            }
        }
    }
    hits
}

/// The name in letters cut from blocks.
const WORDMARK: [&str; 3] = [
    "█▀▀▄ ▄▀▀▄ █▄ ▄█ ▀█▀ █▄  █ ▄▀▀▄ ▄▀▀▀",
    "█  █ █  █ █ ▀ █  █  █ ▀▄█ █  █  ▀▀▄",
    "█▄▄▀ ▀▄▄▀ █   █ ▄█▄ █   █ ▀▄▄▀ ▄▄▄▀",
];

/// The name, with a few tiles under it if there is room; returns the row
/// below.
fn title(buf: &mut Buffer, w: i32, top: i32, tiles: bool) -> i32 {
    for (i, row) in WORDMARK.iter().enumerate() {
        centred(buf, 0, top + i as i32, w, row, plain(IVORY));
    }
    if !tiles {
        return top + 4;
    }
    // as if just played
    let x = w / 2 - 19;
    draw::large(buf, x, top + 5, false, 5, 3, Look::Normal);
    draw::large(buf, x + 12, top + 3, true, 3, 3, Look::Normal);
    draw::large(buf, x + 18, top + 5, false, 3, 6, Look::Last);
    draw::large(buf, x + 30, top + 5, false, 6, 1, Look::Normal);
    top + 11
}

fn menu(app: &App, buf: &mut Buffer, w: i32, h: i32, hits: &mut Hits) {
    let roomy = h >= 32;
    let top = if roomy { (h - 32) / 2 + 1 } else { 1 };
    let y0 = title(buf, w, top, roomy) + 1;
    let s = &app.settings;
    let rules = rules::ALL[s.rules];
    let x = w / 2 - 22;
    let mut y = y0;
    let mut row = |buf: &mut Buffer, i: usize, label: &str, value: &str, y: i32, live: bool| {
        let on = app.menu_row == i;
        let style = if on { bold(plain(IVORY)) } else { plain(MUTED) };
        put(buf, x, y, label, style);
        let shown = match (on, live) {
            (true, true) => format!("◂ {value} ▸"),
            (true, false) => format!("  {value}▏"),
            _ => format!("  {value}  "),
        };
        put(buf, x + 13, y, &shown, if on { chosen() } else { style });
        hits.rows.push((x, y, 44, 1, i));
    };
    row(buf, 0, "Game", rules.name, y, true);
    put(buf, x + 2, y + 1, rules.blurb, plain(TEXT));
    let source = format!("Rules: {}", rules.source);
    put(
        buf,
        x + 2,
        y + 2,
        &source,
        plain(MUTED).add_modifier(Modifier::ITALIC),
    );
    y += 4;
    row(buf, 1, "Players", &s.players.to_string(), y, true);
    let partners = match (s.players, s.teams) {
        (4, true) => "yes, opposite",
        (4, false) => "no",
        _ => "only with 4",
    };
    row(buf, 2, "Partners", partners, y + 1, s.players == 4);
    row(buf, 3, "Computer", s.strength.name(), y + 2, true);
    row(buf, ROW_NAME, "Your name", &app.name, y + 3, false);
    y += 5;
    let buttons = [
        (ROW_PLAY, "Play against the computer"),
        (ROW_HOST, "Host a table on the network"),
        (ROW_JOIN, "Join a table on the network"),
    ];
    for (i, label) in buttons {
        let on = app.menu_row == i;
        let text = if on {
            format!("▸ {label} ◂")
        } else {
            format!("  {label}  ")
        };
        let style = if on { chosen() } else { banded(TEXT) };
        centred(buf, 0, y, w, &text, style);
        hits.rows.push((w / 2 - 16, y, 32, 1, i));
        y += 1;
    }
    if let Some(notice) = &app.notice {
        centred(buf, 0, y + 1, w, notice, plain(GOLD));
    }
    let keys = if app.menu_row == ROW_NAME {
        "type your name   Enter done   ↑↓ choose"
    } else {
        "↑↓ choose   ←→ change   Enter go   Q quit"
    };
    centred(buf, 0, h - 1, w, keys, plain(MUTED));
}

/// A line on a table found or waiting: the game, the players.
fn about(kind: Kind, players: usize, teams: bool) -> String {
    let partners = if teams { ", partners" } else { "" };
    format!("{} · {players} players{partners}", rules::of(kind).name)
}

fn join(app: &App, buf: &mut Buffer, w: i32, h: i32, hits: &mut Hits) {
    let mut y = title(buf, w, 1, false) + 1;
    centred(
        buf,
        0,
        y,
        w,
        "Join a table on the network",
        bold(plain(GOLD)),
    );
    y += 2;
    let found = app
        .finder
        .as_ref()
        .map(|f| f.found.as_slice())
        .unwrap_or(&[]);
    let x = (w / 2 - 34).max(1);
    if found.is_empty() {
        centred(
            buf,
            0,
            y,
            w,
            "Looking for tables on the local network…",
            plain(MUTED),
        );
        y += 2;
    }
    for (i, f) in found.iter().enumerate() {
        let a = &f.advert;
        let open = match a.open {
            0 => "full".to_string(),
            1 => "1 seat open".to_string(),
            n => format!("{n} seats open"),
        };
        let text = format!(
            "{}'s table · {} · {open} · {}",
            a.host,
            about(a.kind, a.players, a.teams),
            f.addr
        );
        let on = app.join_row == i && app.typing.is_none();
        let style = if on { chosen() } else { plain(TEXT) };
        put(buf, x, y, &format!(" {text} "), style);
        hits.rows.push((x, y, w - 2 * x, 1, i));
        y += 1;
    }
    y += 1;
    let row = found.len();
    match &app.typing {
        Some(text) => {
            put(buf, x, y, "Address: ", bold(plain(IVORY)));
            put(buf, x + 9, y, &format!("{text}▏"), chosen());
        }
        None => {
            let on = app.join_row == row;
            let style = if on { chosen() } else { plain(MUTED) };
            put(buf, x, y, " Type an address… ", style);
            hits.rows.push((x, y, 20, 1, row));
        }
    }
    if let Some(notice) = &app.notice {
        centred(buf, 0, y + 2, w, notice, plain(GOLD));
    }
    let keys = if app.typing.is_some() {
        format!(
            "a name or number, :port if not {}   Enter join   Esc back",
            net::PORT
        )
    } else {
        "↑↓ choose   Enter join   A type an address   Esc back".to_string()
    };
    centred(buf, 0, h - 1, w, &keys, plain(MUTED));
}

fn lobby(app: &App, buf: &mut Buffer, w: i32, h: i32) {
    let (lobby, host_here, me): (Option<Lobby>, bool, Option<usize>) = match &app.session {
        Session::Hosting(l) => (Some(l.lobby()), true, Some(0)),
        Session::Guest(g) => (g.lobby.clone(), false, g.seat),
        _ => (None, false, None),
    };
    let mut y = title(buf, w, 1, false) + 1;
    let Some(lobby) = lobby else {
        centred(buf, 0, y, w, "Joining…", plain(MUTED));
        centred(buf, 0, h - 1, w, "Esc back", plain(MUTED));
        return;
    };
    let host = lobby.names[0].clone().unwrap_or_default();
    centred(buf, 0, y, w, &format!("{host}'s table"), bold(plain(GOLD)));
    centred(
        buf,
        0,
        y + 1,
        w,
        &about(lobby.kind, lobby.players, lobby.teams),
        plain(TEXT),
    );
    y += 3;
    let x = w / 2 - 22;
    for (seat, name) in lobby.names.iter().enumerate() {
        let partner = if lobby.teams {
            if seat % 2 == 0 { "  ◆" } else { "  ◇" }
        } else {
            ""
        };
        let (text, style) = match name {
            Some(n) if Some(seat) == me => (format!("{n} (you)"), bold(plain(IVORY))),
            Some(n) => (n.clone(), plain(IVORY)),
            None => (
                "open: the computer plays it unless someone sits".into(),
                plain(MUTED),
            ),
        };
        let picked = host_here && lobby.players > 2 && seat == app.lobby_seat;
        let (text, style) = if picked {
            (format!("{text} ◂▸"), chosen())
        } else {
            (text, style)
        };
        let marker = if picked { "▸" } else { " " };
        put(buf, x - 2, y, marker, bold(plain(CRIMSON)));
        put(
            buf,
            x,
            y,
            &format!("Seat {}{partner}", seat + 1),
            plain(MUTED),
        );
        put(buf, x + 10, y, &text, style);
        y += 1;
    }
    if lobby.teams {
        put(
            buf,
            x,
            y,
            "◆ and ◇ mark the partners, who sit opposite.",
            plain(MUTED),
        );
        y += 1;
    }
    y += 1;
    if let Session::Hosting(l) = &app.session {
        let ip = net::local_ip().map_or("this machine's address".to_string(), |ip| ip.to_string());
        let how = format!(
            "Others choose Join on their menu, or type {ip}:{}",
            l.server.port
        );
        centred(buf, 0, y, w, &how, plain(TEXT));
    } else {
        centred(
            buf,
            0,
            y,
            w,
            &format!("Waiting for {host} to deal…"),
            plain(TEXT),
        );
    }
    for (i, line) in app.log.iter().rev().take(3).enumerate() {
        centred(buf, 0, y + 2 + i as i32, w, line, plain(MUTED));
    }
    let keys = if host_here && lobby.players > 2 {
        "↑↓ choose a seat   ←→ move them   Enter deal   Esc close the table"
    } else if host_here {
        "Enter deal   Esc close the table"
    } else {
        "Esc leave"
    };
    centred(buf, 0, h - 1, w, keys, plain(MUTED));
}

fn table(app: &App, view: &View, buf: &mut Buffer, w: i32, h: i32, hits: &mut Hits) {
    let rules = view.rules();
    let n = view.players;
    let score = |seat: usize| view.scores[view.side(seat)];

    // header: name, game, the boneyard
    fill(buf, 0, 0, w, 1, FELT_DARK);
    put(buf, 0, 0, " DOMINOS ", chosen());
    let partners = if view.teams { " · partners" } else { "" };
    let what = format!(
        " {}{partners} · to {} · hand {}",
        rules.name, view.target, view.hand_no
    );
    put(buf, 9, 0, &what, banded(TEXT));
    if rules.drawing != Drawing::None {
        let text = format!(
            "boneyard {} {} ",
            backs(view.boneyard.min(14)),
            view.boneyard
        );
        let x = w - text.chars().count() as i32;
        put(buf, x, 0, &text, banded(TEXT));
        hits.boneyard = Some((x, 0, w - x, 1));
    }

    // the others, round to the left, each with their tiles face down
    fill(buf, 0, 1, w, 1, FELT_DARK);
    let cell = w / (n as i32 - 1);
    for place in 1..n {
        let seat = (view.seat + place) % n;
        let x = (place as i32 - 1) * cell + 1;
        let turn = view.turn == seat && view.playing();
        put(
            buf,
            x,
            1,
            if turn { "▸ " } else { "  " },
            bold(banded(CRIMSON)),
        );
        let mut name = app.name(view, seat);
        if view.teams && view.side(seat) == view.side(view.seat) {
            name.push_str(" (partner)");
        }
        let name_style = if turn {
            bold(banded(TEXT))
        } else {
            banded(TEXT)
        };
        put(buf, x + 2, 1, &name, name_style);
        let bx = x + 3 + name.chars().count() as i32;
        let count = view.counts[seat];
        put(buf, bx, 1, &backs(count), banded(IVORY_DIM));
        put(
            buf,
            bx + count as i32 + 1,
            1,
            &format!("{:>3}", score(seat)),
            banded(GOLD),
        );
    }

    // the person's hand: large tiles standing, or small ones when many
    let hand = &view.hand;
    let large = hand.len() as i32 * 6 + 2 <= w;
    let hand_h = if large { 9 } else { 5 };
    let hand_y = h - 1 - hand_h;
    let board_y = 3;
    let board_h = hand_y - 3 - board_y;

    // the layout
    let (ghosts, chosen_end) = app.ghosts(view);
    let lay = layout::layout(&view.board, &ghosts, w - 2, board_h);
    for p in &lay.tiles {
        let look = match p.ghost {
            Some(arm) if Some(arm) == chosen_end => Look::Chosen,
            Some(_) => Look::Ghost,
            None if Some(p.tile()) == app.last => Look::Last,
            None => Look::Normal,
        };
        tile(buf, p, 1, board_y, lay.size.is_large(), look);
        if let Some(arm) = p.ghost {
            hits.ends.push((p.x + 1, p.y + board_y, p.w, p.h, arm));
        }
    }
    if rules.kind == Kind::AllFives && !view.board.is_empty() {
        let count = view.board.count();
        let text = format!(" ends {count} ");
        let style = if count.is_multiple_of(5) {
            bold(Style::new().fg(FELT_DARK).bg(BONE_LAST))
        } else {
            plain(MUTED)
        };
        put(buf, w - text.chars().count() as i32 - 1, 2, &text, style);
    }

    // the last things said
    let log_y = hand_y - 2;
    for (i, line) in app.log.iter().rev().take(2).enumerate() {
        let style = if i == 0 { plain(TEXT) } else { plain(MUTED) };
        put(buf, 2, log_y + 1 - i as i32, line, style);
    }

    // the hand
    let mine = app.your_turn(view);
    let legal = if mine { view.legal() } else { Vec::new() };
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
        if picked {
            let bar = if large {
                "▀▀▀▀▀"
            } else {
                "▀▀▀"
            };
            put(buf, x, y + if large { 7 } else { 3 }, bar, plain(CRIMSON));
        }
        hits.hand.push((x, hand_y, step - 1, hand_h, i));
    }

    // footer: the person's score and what to do
    let you_turn = view.turn == view.seat && view.playing();
    fill(buf, 0, h - 1, w, 1, FELT_DARK);
    put(
        buf,
        1,
        h - 1,
        if you_turn { "▸ " } else { "  " },
        bold(banded(CRIMSON)),
    );
    let me = app.side_name(view, view.side(view.seat));
    put(buf, 3, h - 1, &me, bold(banded(TEXT)));
    let mx = 4 + me.chars().count() as i32;
    put(
        buf,
        mx,
        h - 1,
        &format!("{:>3}", score(view.seat)),
        banded(GOLD),
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
    put(
        buf,
        w - hint.chars().count() as i32 - 1,
        h - 1,
        &hint,
        banded(MUTED),
    );

    if app.help {
        help_box(view, buf, w, h);
    } else {
        match &view.phase {
            Phase::Playing => {}
            Phase::HandOver(result) => hand_over(app, view, buf, w, h, result),
            Phase::GameOver { winner } => game_over(app, view, buf, w, h, *winner),
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
    let edge = banded(GOLD);
    let line = "─".repeat(bw as usize - 2);
    put(buf, x, y, &format!("╭{line}╮"), edge);
    put(buf, x, y + bh - 1, &format!("╰{line}╯"), edge);
    for r in 1..bh - 1 {
        put(buf, x, y + r, "│", edge);
        put(buf, x + bw - 1, y + r, "│", edge);
    }
    centred(buf, x, y, bw, &format!(" {title} "), chosen());
    (x + 2, y + 1)
}

fn help_box(view: &View, buf: &mut Buffer, w: i32, h: i32) {
    let lines: Vec<&str> = help::rules(view.kind)
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
            bold(banded(GOLD))
        } else {
            banded(TEXT)
        };
        put(buf, x, y + 1 + i as i32, line, style);
    }
}

/// What the person presses to go on, or who everyone waits for.
fn go_on(app: &App, view: &View, what: &str) -> String {
    if app.demo {
        return String::new();
    }
    if view.waiting.contains(&view.seat) {
        return format!("Enter: {what}");
    }
    let names: Vec<String> = view.waiting.iter().map(|&s| app.name(view, s)).collect();
    if names.is_empty() {
        String::new()
    } else {
        format!("Waiting for {}…", names.join(", "))
    }
}

fn hand_over(app: &App, view: &View, buf: &mut Buffer, w: i32, h: i32, r: &HandResult) {
    let n = view.players;
    let bw = 70.min(w);
    let bh = n as i32 * 2 + 8;
    let title = match (r.dominoed, r.winner) {
        (Some(p), _) => format!("{} played out", app.name(view, p)),
        (None, Some(side)) => format!("Blocked · {} the fewest pips", {
            let who = app.side_name(view, side);
            let have = if view.teams || who == "You" {
                "have"
            } else {
                "has"
            };
            format!("{who} {have}")
        }),
        (None, None) => "Blocked · tied for the fewest".to_string(),
    };
    let (x, y) = frame(buf, w, h, bw, bh, &title);
    put(buf, x + 44, y + 1, "pips   hand   game", banded(MUTED));
    for place in 0..n {
        let seat = (view.seat + place) % n;
        let side = view.side(seat);
        let row = y + 2 + place as i32 * 2;
        let style = banded(TEXT);
        let strong = if r.winner == Some(side) {
            bold(style)
        } else {
            style
        };
        put(buf, x, row, &app.name(view, seat), strong);
        let mut tx = x + 11;
        for &t in r.hands[seat].iter().take(5) {
            draw::small(buf, tx, row, false, t.hi, t.lo, Look::Normal);
            tx += 6;
        }
        if r.hands[seat].len() > 5 {
            put(buf, tx, row, "…", style);
        }
        put(buf, x + 44, row, &format!("{:>4}", r.pips[seat]), style);
        let points = r.points[side];
        let (text, pstyle) = if points > 0 {
            (format!("+{points}"), bold(banded(GOLD)))
        } else {
            ("·".into(), style)
        };
        put(buf, x + 51, row, &format!("{text:>4}"), pstyle);
        put(
            buf,
            x + 58,
            row,
            &format!("{:>4}", view.scores[side]),
            strong,
        );
    }
    let note = match (view.rules().hand_score, view.teams) {
        (HandScore::LowestTakesDifference, true) => {
            "Partners count together; the winners score the losers' pips."
        }
        (HandScore::LowestTakesDifference, false) => {
            "The lowest count scores the others' pips less its own."
        }
        (HandScore::OpponentsRounded, _) => {
            "The winners score the others' pips, to the nearest five."
        }
    };
    put(buf, x, y + bh - 4, note, banded(MUTED));
    let next = go_on(app, view, "next hand");
    centred(buf, x - 2, y + bh - 3, bw, &next, bold(banded(IVORY)));
}

fn game_over(app: &App, view: &View, buf: &mut Buffer, w: i32, h: i32, winner: usize) {
    let sides = view.scores.len();
    let bw = 48.min(w);
    let bh = sides as i32 + 7;
    let who = app.side_name(view, winner);
    let title = if who.starts_with("You") {
        format!("{who} win the game!")
    } else if view.teams {
        format!("{who} win the game")
    } else {
        format!("{who} wins the game")
    };
    let (x, y) = frame(buf, w, h, bw, bh, &title);
    let mut order: Vec<usize> = (0..sides).collect();
    order.sort_by_key(|&s| std::cmp::Reverse(view.scores[s]));
    for (i, &side) in order.iter().enumerate() {
        let style = if side == winner {
            bold(banded(GOLD))
        } else {
            banded(TEXT)
        };
        put(
            buf,
            x + 4,
            y + 2 + i as i32,
            &app.side_name(view, side),
            style,
        );
        put(
            buf,
            x + 32,
            y + 2 + i as i32,
            &format!("{:>4}", view.scores[side]),
            style,
        );
    }
    let next = go_on(app, view, "play again · Q: menu");
    centred(buf, x - 2, y + bh - 3, bw, &next, bold(banded(IVORY)));
}
