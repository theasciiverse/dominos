//! The program's state between frames: the menu, finding and hosting
//! tables, the game being played (here or on another machine), and what
//! the person has picked.

use crate::ai::Strength;
use crate::board::Arm;
use crate::draw::named;
use crate::game::{Action, Event};
use crate::net::{self, Connection, Finder, Lobby, ToGuest, ToHost};
use crate::rules::Drawing;
use crate::table::{self, HostLobby, Settings, Sitter, Table, Update};
use crate::tile::Tile;
use crate::view::View;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Menu,
    /// Looking for a table on the network.
    Join,
    /// Seated at a network table, before the deal.
    Lobby,
    Table,
}

/// The menu's rows.
pub const MENU_ROWS: usize = 8;
pub const ROW_NAME: usize = 4;
pub const ROW_PLAY: usize = 5;
pub const ROW_HOST: usize = 6;
pub const ROW_JOIN: usize = 7;

/// Where the game is.
pub enum Session {
    None,
    /// Run here: alone with the computer, or hosting.
    Here(Table),
    Hosting(HostLobby),
    /// Run by a host on the network.
    Guest(Guest),
}

pub struct Guest {
    pub conn: Connection,
    pub host: String,
    pub seat: Option<usize>,
    pub lobby: Option<Lobby>,
    pub view: Option<View>,
}

/// The places to click, from the last frame drawn.
#[derive(Clone, Debug, Default)]
pub struct Hits {
    pub hand: Vec<(i32, i32, i32, i32, usize)>,
    pub ends: Vec<(i32, i32, i32, i32, Arm)>,
    pub boneyard: Option<(i32, i32, i32, i32)>,
    pub rows: Vec<(i32, i32, i32, i32, usize)>,
}

fn inside(x: i32, y: i32, r: (i32, i32, i32, i32)) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}

pub struct App {
    pub screen: Screen,
    pub session: Session,
    pub settings: Settings,
    pub name: String,
    pub menu_row: usize,
    pub finder: Option<Finder>,
    pub join_row: usize,
    /// The seat picked in the host's lobby, to move whoever sits there.
    pub lobby_seat: usize,
    /// An address being typed on the Join screen.
    pub typing: Option<String>,
    /// A word for the person on the menu or Join screen.
    pub notice: Option<String>,
    pub demo: bool,
    /// The tile picked in the hand.
    pub cursor: usize,
    /// Choosing between ends for the picked tile: the ends, and which one.
    pub choosing: Option<(Vec<Arm>, usize)>,
    pub log: Vec<String>,
    pub last: Option<Tile>,
    pub help: bool,
    pub hits: Hits,
    pub quit: bool,
    seed: u64,
}

impl App {
    pub fn new(seed: u64, rules: usize, demo: bool) -> App {
        let name = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .map(|n| table::clean(&n))
            .unwrap_or_else(|_| "Guest".into());
        let mut app = App {
            screen: Screen::Menu,
            session: Session::None,
            settings: Settings {
                rules,
                players: if demo { 4 } else { 2 },
                teams: false,
                strength: Strength::Normal,
            },
            name,
            menu_row: ROW_PLAY,
            finder: None,
            join_row: 0,
            lobby_seat: 1,
            typing: None,
            notice: None,
            demo,
            cursor: 0,
            choosing: None,
            log: Vec::new(),
            last: None,
            help: false,
            hits: Hits::default(),
            quit: false,
            seed,
        };
        if demo {
            app.play_here();
        }
        app
    }

    /// The game as this seat sees it.
    pub fn view(&self) -> Option<View> {
        match &self.session {
            Session::Here(table) => Some(table.view(0)),
            Session::Guest(guest) => guest.view.clone(),
            _ => None,
        }
    }

    /// The name for `seat`: "You" here, people by their names, computer
    /// players by where they sit.
    pub fn name(&self, view: &View, seat: usize) -> String {
        let place = view.place(seat);
        if place == 0 && !self.demo {
            return "You".into();
        }
        if let Some(name) = &view.names[seat] {
            return name.clone();
        }
        let compass: &[&str] = match view.players {
            2 => &["South", "North"],
            3 => &["South", "West", "East"],
            _ => &["South", "West", "North", "East"],
        };
        compass[place].into()
    }

    /// The name for a side: a seat's, or a pair of partners'.
    pub fn side_name(&self, view: &View, side: usize) -> String {
        if !view.teams {
            return self.name(view, side);
        }
        let (a, b) = (side, side + 2);
        // put "You" first
        let (a, b) = if view.place(b) == 0 { (b, a) } else { (a, b) };
        format!("{} and {}", self.name(view, a), self.name(view, b))
    }

    fn reset_table(&mut self) {
        self.log.clear();
        self.last = None;
        self.cursor = 0;
        self.choosing = None;
        self.help = false;
    }

    fn play_here(&mut self) {
        let players = self.settings.players;
        let sitters = (0..players)
            .map(|seat| match seat {
                0 if self.demo => Sitter::Computer(Strength::Hard),
                0 => Sitter::Here,
                _ => Sitter::Computer(self.settings.strength),
            })
            .collect();
        let (table, update) = Table::new(
            self.settings,
            sitters,
            vec![None; players],
            None,
            self.next_seed(),
            self.demo,
        );
        self.reset_table();
        self.session = Session::Here(table);
        self.screen = Screen::Table;
        self.absorb(update);
    }

    fn next_seed(&mut self) -> u64 {
        let seed = self.seed;
        self.seed = self.seed.wrapping_add(1000);
        seed
    }

    fn host(&mut self) {
        match HostLobby::open(self.settings, &self.name) {
            Ok(lobby) => {
                self.reset_table();
                self.lobby_seat = 1;
                if !lobby.server.findable {
                    self.log.push(
                        "Another table on this machine answers searches: players type the address."
                            .into(),
                    );
                }
                self.session = Session::Hosting(lobby);
                self.screen = Screen::Lobby;
            }
            Err(e) => self.notice = Some(format!("Could not open a table: {e}")),
        }
    }

    fn join_screen(&mut self) {
        self.finder = Some(Finder::new());
        self.join_row = 0;
        self.typing = None;
        self.notice = None;
        self.screen = Screen::Join;
    }

    fn connect(&mut self, addr: std::net::SocketAddr, host: String) {
        match Connection::connect(addr, &self.name) {
            Ok(conn) => {
                self.reset_table();
                self.session = Session::Guest(Guest {
                    conn,
                    host,
                    seat: None,
                    lobby: None,
                    view: None,
                });
                self.finder = None;
                self.notice = None;
                self.screen = Screen::Lobby;
            }
            Err(e) => self.notice = Some(format!("Could not reach {addr}: {e}")),
        }
    }

    /// Back to the menu, closing or leaving any table.
    fn leave(&mut self, notice: Option<String>) {
        match &mut self.session {
            Session::Here(table) => table.close("The host closed the table."),
            Session::Hosting(lobby) => lobby.close("The host closed the table."),
            _ => {}
        }
        self.session = Session::None;
        self.finder = None;
        self.typing = None;
        self.notice = notice;
        self.screen = Screen::Menu;
    }

    /// Whether the person is to move now.
    pub fn your_turn(&self, view: &View) -> bool {
        self.screen == Screen::Table && !self.demo && !self.help && view.my_turn()
    }

    /// The tile under the cursor and where it can go, if it can be played.
    pub fn picked(&self, view: &View) -> Option<(Tile, Vec<Arm>)> {
        let tile = *view.hand.get(self.cursor)?;
        if !self.your_turn(view) {
            return None;
        }
        let places: Vec<Arm> = view
            .legal()
            .into_iter()
            .filter_map(|a| match a {
                Action::Play(t, arm) if t == tile => Some(arm),
                _ => None,
            })
            .collect();
        (!places.is_empty()).then_some((tile, places))
    }

    /// The tiles to show where they could go: the picked tile at each end
    /// it fits, and which of them is chosen.
    pub fn ghosts(&self, view: &View) -> (Vec<(Arm, Tile)>, Option<Arm>) {
        let Some((tile, places)) = self.picked(view) else {
            return (Vec::new(), None);
        };
        let chosen = self.choosing.as_ref().map(|(ends, i)| ends[*i]);
        let chosen = chosen.or_else(|| (places.len() == 1).then(|| places[0]));
        (places.iter().map(|&a| (a, tile)).collect(), chosen)
    }

    fn act(&mut self, action: Action) {
        self.choosing = None;
        match &mut self.session {
            Session::Here(table) => match table.act(0, action) {
                Ok(update) => self.absorb(update),
                Err(e) => log::warn!("{e}"),
            },
            Session::Guest(guest) => {
                if let Err(e) = guest.conn.send(&ToHost::Act(action)) {
                    log::warn!("{e}");
                }
                // what the host says comes back as the new view; until
                // then, nothing more to play
                if let Some(view) = &mut guest.view {
                    view.turn = usize::MAX;
                }
            }
            _ => {}
        }
    }

    /// Ready for the next hand or game.
    fn ready(&mut self) {
        match &mut self.session {
            Session::Here(table) => {
                let update = table.ready(0);
                self.absorb(update);
            }
            Session::Guest(guest) => {
                let _ = guest.conn.send(&ToHost::Ready);
            }
            _ => {}
        }
    }

    /// Take in what changed: messages for the log, the last tile played.
    fn absorb(&mut self, update: Update) {
        let Some(view) = self.view() else { return };
        let you = |seat: usize| view.place(seat) == 0 && !self.demo;
        for e in &update.events {
            log::debug!("{e:?}");
            let msg = match e {
                Event::Dealt { who, by_lot } => {
                    self.last = None;
                    self.choosing = None;
                    let leads = if you(*who) { "lead" } else { "leads" };
                    let who = self.name(&view, *who);
                    if *by_lot {
                        format!(
                            "Hand {}. The lot falls on {who}: {who} {leads}.",
                            view.hand_no
                        )
                    } else {
                        format!("Hand {}. {who} {leads}.", view.hand_no)
                    }
                }
                Event::Played {
                    who,
                    tile,
                    arm,
                    scored,
                } => {
                    self.last = Some(*tile);
                    let verb = if you(*who) { "play" } else { "plays" };
                    let at = if view.board.len() == 1 {
                        String::new()
                    } else {
                        format!(" on the {}", arm.name())
                    };
                    let score = if *scored > 0 {
                        format!("  ✦ {scored} points")
                    } else {
                        String::new()
                    };
                    format!(
                        "{} {verb} {}{at}.{score}",
                        self.name(&view, *who),
                        named(*tile)
                    )
                }
                Event::Drew { who, tile } => match tile {
                    Some(t) if you(*who) => format!("You draw {}.", named(*t)),
                    _ => format!("{} draws.", self.name(&view, *who)),
                },
                Event::Passed { who } => {
                    if you(*who) {
                        "You pass.".into()
                    } else {
                        format!("{} passes.", self.name(&view, *who))
                    }
                }
                Event::HandOver(result) => match (result.dominoed, result.winner) {
                    (Some(w), _) => format!("{} played out.", self.name(&view, w)),
                    (None, Some(side)) => {
                        let who = self.side_name(&view, side);
                        let have = if view.teams || who == "You" {
                            "have"
                        } else {
                            "has"
                        };
                        format!("Blocked. {who} {have} the fewest pips.")
                    }
                    (None, None) => "Blocked, and a tie for the fewest pips.".into(),
                },
                Event::GameOver { winner } => {
                    format!("{} won the game.", self.side_name(&view, *winner))
                }
            };
            self.log.push(msg);
        }
        self.log.extend(update.notes);
        if self.log.len() > 50 {
            self.log.drain(..self.log.len() - 50);
        }
        let only_pass = view.legal() == vec![Action::Pass];
        if !update.events.is_empty() && self.your_turn(&view) && only_pass {
            self.log.push("You cannot play.".into());
        }
        if !view.my_turn() {
            self.choosing = None;
        }
        self.cursor = self.cursor.min(view.hand.len().saturating_sub(1));
        if let Some(m) = view.must_play {
            self.cursor = view.hand.iter().position(|&t| t == m).unwrap_or(0);
        }
    }

    /// Time passes: the table moves on, the network brings news.
    pub fn tick(&mut self, now: Instant) {
        if let Some(finder) = &mut self.finder {
            finder.poll(now);
            self.join_row = self.join_row.min(finder.found.len());
        }
        match &mut self.session {
            Session::Here(table) => {
                let update = table.tick(now);
                self.absorb(update);
            }
            Session::Hosting(lobby) => {
                let notes = lobby.poll();
                self.log.extend(notes);
            }
            Session::Guest(guest) => {
                let messages = guest.conn.poll();
                for msg in messages {
                    self.hear(msg);
                }
            }
            Session::None => {}
        }
    }

    fn hear(&mut self, msg: Option<ToGuest>) {
        let Session::Guest(guest) = &mut self.session else {
            return;
        };
        match msg {
            Some(ToGuest::Welcome { seat }) => guest.seat = Some(seat),
            Some(ToGuest::Lobby(lobby)) => guest.lobby = Some(lobby),
            Some(ToGuest::State {
                view,
                events,
                notes,
            }) => {
                guest.view = Some(*view);
                self.screen = Screen::Table;
                self.absorb(Update { events, notes });
            }
            Some(ToGuest::Refused(why)) => {
                if guest.view.is_some() {
                    self.log.push(why);
                } else {
                    self.leave(Some(why));
                }
            }
            Some(ToGuest::Closed(why)) => self.leave(Some(why)),
            None => {
                let why = format!("Lost the connection to {}.", guest.host);
                self.leave(Some(why));
            }
        }
    }

    pub fn key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.leave(None);
            self.quit = true;
            return;
        }
        match self.screen {
            Screen::Menu => self.menu_key(key.code),
            Screen::Join => self.join_key(key.code),
            Screen::Lobby => self.lobby_key(key.code),
            Screen::Table => self.table_key(key.code),
        }
    }

    fn change(&mut self, step: i32) {
        let s = &mut self.settings;
        match self.menu_row {
            0 => s.rules = (s.rules as i32 + step).rem_euclid(3) as usize,
            1 => s.players = ((s.players as i32 - 2 + step).rem_euclid(3) + 2) as usize,
            2 if s.players == 4 => s.teams = !s.teams,
            3 => {
                let i = Strength::ALL
                    .iter()
                    .position(|&x| x == s.strength)
                    .unwrap_or(1);
                s.strength = Strength::ALL[(i as i32 + step).rem_euclid(3) as usize];
            }
            _ => {}
        }
    }

    fn menu_key(&mut self, code: KeyCode) {
        if self.menu_row == ROW_NAME {
            match code {
                KeyCode::Char(c) if !c.is_control() => {
                    if self.name.chars().count() < 12 {
                        self.name.push(c);
                    }
                    return;
                }
                KeyCode::Backspace => {
                    self.name.pop();
                    return;
                }
                KeyCode::Esc | KeyCode::Enter => {
                    self.name = table::clean(&self.name);
                    self.menu_row += 1;
                    return;
                }
                _ => self.name = table::clean(&self.name),
            }
        }
        self.notice = None;
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.menu_row = (self.menu_row + MENU_ROWS - 1) % MENU_ROWS
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.menu_row = (self.menu_row + 1) % MENU_ROWS
            }
            KeyCode::Left | KeyCode::Char('h') => self.change(-1),
            KeyCode::Right | KeyCode::Char('l') => self.change(1),
            KeyCode::Enter | KeyCode::Char(' ') => match self.menu_row {
                ROW_HOST => self.host(),
                ROW_JOIN => self.join_screen(),
                _ => self.play_here(),
            },
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            _ => {}
        }
    }

    fn join_key(&mut self, code: KeyCode) {
        let found = self.finder.as_ref().map_or(0, |f| f.found.len());
        if let Some(text) = &mut self.typing {
            match code {
                KeyCode::Char(c) if !c.is_control() && text.len() < 64 => text.push(c),
                KeyCode::Backspace => {
                    text.pop();
                }
                KeyCode::Esc => self.typing = None,
                KeyCode::Enter => {
                    let text = text.trim().to_string();
                    match net::resolve(&text) {
                        Ok(addr) => self.connect(addr, text),
                        Err(e) => self.notice = Some(format!("{text}: {e}")),
                    }
                }
                _ => {}
            }
            return;
        }
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.join_row = (self.join_row + found) % (found + 1)
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.join_row = (self.join_row + 1) % (found + 1)
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let pick = self
                    .finder
                    .as_ref()
                    .and_then(|f| f.found.get(self.join_row))
                    .map(|f| (f.addr, f.advert.host.clone()));
                match pick {
                    Some((addr, host)) => self.connect(addr, host),
                    None => self.typing = Some(String::new()),
                }
            }
            KeyCode::Char('a') => self.typing = Some(String::new()),
            KeyCode::Char('q') | KeyCode::Esc => self.leave(None),
            _ => {}
        }
    }

    fn lobby_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                if !matches!(self.session, Session::Hosting(_)) {
                    return;
                }
                let Session::Hosting(lobby) = std::mem::replace(&mut self.session, Session::None)
                else {
                    unreachable!()
                };
                let (table, update) = lobby.start(self.next_seed());
                self.session = Session::Here(table);
                self.screen = Screen::Table;
                self.absorb(update);
            }
            KeyCode::Char('q') | KeyCode::Esc => self.leave(None),
            _ => self.seat_key(code),
        }
    }

    /// The host picks a seat and moves whoever sits there; seat 0 is the
    /// host's own.
    fn seat_key(&mut self, code: KeyCode) {
        let Session::Hosting(lobby) = &mut self.session else {
            return;
        };
        let others = lobby.names.len() - 1;
        if others < 2 {
            return;
        }
        let along = |seat: usize, by: usize| (seat - 1 + by) % others + 1;
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.lobby_seat = along(self.lobby_seat, others - 1)
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.lobby_seat = along(self.lobby_seat, 1)
            }
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Right | KeyCode::Char('l') => {
                let by = if matches!(code, KeyCode::Left | KeyCode::Char('h')) {
                    others - 1
                } else {
                    1
                };
                let to = along(self.lobby_seat, by);
                lobby.swap(self.lobby_seat, to);
                self.lobby_seat = to;
            }
            _ => {}
        }
    }

    fn table_key(&mut self, code: KeyCode) {
        if self.help {
            self.help = false;
            return;
        }
        let Some(view) = self.view() else { return };
        if !view.playing() {
            match code {
                KeyCode::Enter | KeyCode::Char(' ') if !self.demo => self.ready(),
                KeyCode::Char('q') | KeyCode::Esc => self.quit_table(),
                KeyCode::Char('?') | KeyCode::F(1) => self.help = true,
                _ => {}
            }
            return;
        }
        match code {
            KeyCode::Char('?') | KeyCode::F(1) => self.help = true,
            KeyCode::Esc if self.choosing.is_some() => self.choosing = None,
            KeyCode::Char('q') | KeyCode::Esc => self.quit_table(),
            _ if !self.your_turn(&view) => {}
            KeyCode::Left | KeyCode::Char('h') => self.step(&view, -1),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => self.step(&view, 1),
            KeyCode::Up | KeyCode::Down => self.step(&view, 1),
            KeyCode::Enter | KeyCode::Char(' ') => self.confirm(&view),
            KeyCode::Char('d') => self.draw(&view),
            _ => {}
        }
    }

    fn quit_table(&mut self) {
        self.leave(None);
        if self.demo {
            self.quit = true;
        }
    }

    /// Move along the hand, or between ends while choosing one.
    fn step(&mut self, view: &View, by: i32) {
        if let Some((ends, i)) = &mut self.choosing {
            *i = (*i as i32 + by).rem_euclid(ends.len() as i32) as usize;
            return;
        }
        let n = view.hand.len() as i32;
        if n > 0 && view.must_play.is_none() {
            self.cursor = (self.cursor as i32 + by).rem_euclid(n) as usize;
        }
    }

    fn confirm(&mut self, view: &View) {
        if let Some((ends, i)) = self.choosing.take() {
            if let Some((tile, _)) = self.picked(view) {
                self.act(Action::Play(tile, ends[i]));
            }
            return;
        }
        match self.picked(view) {
            Some((tile, places)) if places.len() == 1 => self.act(Action::Play(tile, places[0])),
            Some((_, places)) => self.choosing = Some((places, 0)),
            None => {
                if view.legal() == vec![Action::Draw] {
                    self.draw(view);
                } else if self.your_turn(view) {
                    self.log.push("That tile does not fit.".into());
                }
            }
        }
    }

    fn draw(&mut self, view: &View) {
        if view.legal().contains(&Action::Draw) {
            self.act(Action::Draw);
            return;
        }
        let why = match view.rules().drawing {
            Drawing::None => "There is no drawing in the Block game.",
            Drawing::Free { .. } => "The last two tiles in the boneyard stay there.",
            Drawing::Forced if view.boneyard == 0 => "The boneyard is empty.",
            Drawing::Forced => "You can play, so you may not draw.",
        };
        self.log.push(why.into());
    }

    pub fn mouse(&mut self, m: MouseEvent) {
        if m.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        let (x, y) = (m.column as i32, m.row as i32);
        let row = self
            .hits
            .rows
            .iter()
            .find(|r| inside(x, y, (r.0, r.1, r.2, r.3)))
            .map(|r| r.4);
        match self.screen {
            Screen::Menu => {
                let Some(row) = row else { return };
                let again = self.menu_row == row;
                self.menu_row = row;
                if row >= ROW_PLAY {
                    self.menu_key(KeyCode::Enter);
                } else if again && row != ROW_NAME {
                    self.menu_key(KeyCode::Right);
                }
                return;
            }
            Screen::Join => {
                if let Some(row) = row {
                    self.join_row = row;
                    self.join_key(KeyCode::Enter);
                }
                return;
            }
            Screen::Lobby => return,
            Screen::Table => {}
        }
        let Some(view) = self.view() else { return };
        if self.help || !view.playing() {
            self.table_key(KeyCode::Enter);
            return;
        }
        if !self.your_turn(&view) {
            return;
        }
        let end = self
            .hits
            .ends
            .iter()
            .find(|r| inside(x, y, (r.0, r.1, r.2, r.3)));
        if let Some(&(.., arm)) = end {
            if let Some((tile, _)) = self.picked(&view) {
                self.act(Action::Play(tile, arm));
            }
            return;
        }
        let tile = self
            .hits
            .hand
            .iter()
            .find(|r| inside(x, y, (r.0, r.1, r.2, r.3)));
        if let Some(&(.., i)) = tile {
            if view.must_play.is_some() && self.cursor != i {
                return;
            }
            if self.cursor == i {
                self.confirm(&view);
            } else {
                self.cursor = i;
                self.choosing = None;
            }
            return;
        }
        if self.hits.boneyard.is_some_and(|r| inside(x, y, r)) {
            self.draw(&view);
        }
    }
}
