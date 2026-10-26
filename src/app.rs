//! The program's state between frames: the menu, the game, what the
//! person has chosen, and the computer players' timing.

use crate::ai::{self, Strength};
use crate::board::Arm;
use crate::draw::named;
use crate::game::{Action, Event, Game, HandResult, Phase};
use crate::rng::Rng;
use crate::rules::{self, Rules};
use crate::tile::Tile;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::time::{Duration, Instant};

const THINK: Duration = Duration::from_millis(800);
const DRAWING: Duration = Duration::from_millis(350);
const SHOW_PASS: Duration = Duration::from_millis(1400);
const DEMO_PAUSE: Duration = Duration::from_millis(3500);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Menu,
    Table,
}

#[derive(Clone, Debug)]
pub enum Overlay {
    None,
    Help,
    HandOver(HandResult),
    GameOver,
}

/// The menu's choices.
#[derive(Clone, Copy, Debug)]
pub struct Setup {
    pub rules: usize,
    pub opponents: usize,
    pub strength: Strength,
}

impl Setup {
    pub fn rules(&self) -> Rules {
        rules::ALL[self.rules]
    }
}

/// The places to click, from the last frame drawn.
#[derive(Clone, Debug, Default)]
pub struct Hits {
    pub hand: Vec<(i32, i32, i32, i32, usize)>,
    pub ends: Vec<(i32, i32, i32, i32, Arm)>,
    pub boneyard: Option<(i32, i32, i32, i32)>,
    pub menu: Vec<(i32, i32, i32, i32, usize)>,
}

fn inside(x: i32, y: i32, r: (i32, i32, i32, i32)) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}

pub struct App {
    pub screen: Screen,
    pub setup: Setup,
    pub menu_row: usize,
    pub demo: bool,
    pub game: Game,
    /// Who plays each seat: the computer at a strength, or the person.
    pub seats: Vec<Option<Strength>>,
    /// The tile picked in the hand.
    pub cursor: usize,
    /// Choosing between ends for the picked tile: the ends, and which one.
    pub choosing: Option<(Vec<Arm>, usize)>,
    pub log: Vec<String>,
    pub last: Option<Tile>,
    pub overlay: Overlay,
    pub hits: Hits,
    pub quit: bool,
    wait_until: Instant,
    seed: u64,
    rng: Rng,
}

impl App {
    pub fn new(seed: u64, rules: usize, demo: bool) -> App {
        let setup = Setup {
            rules,
            opponents: if demo { 3 } else { 1 },
            strength: Strength::Normal,
        };
        let (game, _) = Game::new(setup.rules(), setup.opponents + 1, seed);
        let mut app = App {
            screen: Screen::Menu,
            setup,
            menu_row: 0,
            demo,
            game,
            seats: Vec::new(),
            cursor: 0,
            choosing: None,
            log: Vec::new(),
            last: None,
            overlay: Overlay::None,
            hits: Hits::default(),
            quit: false,
            wait_until: Instant::now(),
            seed,
            rng: Rng::new(seed ^ 0x5EED),
        };
        if demo {
            app.start();
        }
        app
    }

    pub fn players(&self) -> usize {
        self.game.players
    }

    /// The seats' names, going round to the left from the person.
    pub fn name(&self, seat: usize) -> &'static str {
        let names: &[&str] = match self.players() {
            2 => &["You", "North"],
            3 => &["You", "West", "East"],
            _ => &["You", "West", "North", "East"],
        };
        if seat == 0 && self.demo {
            "South"
        } else {
            names[seat]
        }
    }

    fn start(&mut self) {
        let players = self.setup.opponents + 1;
        let (game, events) = Game::new(self.setup.rules(), players, self.seed);
        self.seed = self.seed.wrapping_add(1);
        self.game = game;
        self.seats = (0..players)
            .map(|i| match i {
                0 if self.demo => Some(Strength::Hard),
                0 => None,
                _ => Some(self.setup.strength),
            })
            .collect();
        self.log.clear();
        self.last = None;
        self.cursor = 0;
        self.choosing = None;
        self.overlay = Overlay::None;
        self.screen = Screen::Table;
        self.wait_until = Instant::now() + THINK;
        self.events(events);
    }

    /// Whether the person is to move now.
    pub fn your_turn(&self) -> bool {
        self.screen == Screen::Table
            && matches!(self.overlay, Overlay::None)
            && matches!(self.game.phase, Phase::Playing)
            && self.seats.get(self.game.turn) == Some(&None)
    }

    /// The tile under the cursor, if it can be played now.
    pub fn picked(&self) -> Option<(Tile, Vec<Arm>)> {
        let tile = *self.game.hands[0].get(self.cursor)?;
        if !self.your_turn() {
            return None;
        }
        let places: Vec<Arm> = self
            .game
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
    pub fn ghosts(&self) -> (Vec<(Arm, Tile)>, Option<Arm>) {
        let Some((tile, places)) = self.picked() else {
            return (Vec::new(), None);
        };
        let chosen = self.choosing.as_ref().map(|(ends, i)| ends[*i]);
        let chosen = chosen.or_else(|| (places.len() == 1).then(|| places[0]));
        (places.iter().map(|&a| (a, tile)).collect(), chosen)
    }

    fn act(&mut self, action: Action) {
        match self.game.apply(action) {
            Ok(events) => self.events(events),
            Err(e) => log::warn!("{e}"),
        }
        self.choosing = None;
    }

    fn events(&mut self, events: Vec<Event>) {
        let mut pause = THINK;
        for e in events {
            log::debug!("{e:?}");
            let msg = match e {
                Event::Dealt { who, by_lot } => {
                    self.last = None;
                    let leads = if who == 0 && !self.demo {
                        "lead"
                    } else {
                        "leads"
                    };
                    if by_lot {
                        format!(
                            "Hand {}. The lot falls on {}: {} {leads}.",
                            self.game.hand_no,
                            self.name(who),
                            self.name(who)
                        )
                    } else {
                        format!("Hand {}. {} {leads}.", self.game.hand_no, self.name(who))
                    }
                }
                Event::Played {
                    who,
                    tile,
                    arm,
                    scored,
                } => {
                    self.last = Some(tile);
                    let verb = if who == 0 && !self.demo {
                        "play"
                    } else {
                        "plays"
                    };
                    let at = if self.game.board.len() == 1 {
                        String::new()
                    } else {
                        format!(" on the {}", arm.name())
                    };
                    let score = if scored > 0 {
                        format!("  ✦ {scored} points")
                    } else {
                        String::new()
                    };
                    format!("{} {verb} {}{at}.{score}", self.name(who), named(tile))
                }
                Event::Drew { who, tile } => {
                    pause = DRAWING;
                    if who == 0 && !self.demo {
                        format!("You draw {}.", named(tile))
                    } else {
                        format!("{} draws.", self.name(who))
                    }
                }
                Event::Passed { who } => {
                    if who == 0 && !self.demo {
                        "You pass.".into()
                    } else {
                        format!("{} passes.", self.name(who))
                    }
                }
                Event::HandOver(result) => {
                    pause = DEMO_PAUSE;
                    let msg = match (result.dominoed, result.winner) {
                        (Some(w), _) => format!("{} played out.", self.name(w)),
                        (None, Some(w)) => {
                            format!("Blocked. {} has the fewest pips.", self.name(w))
                        }
                        (None, None) => "Blocked, and a tie for the fewest pips.".into(),
                    };
                    self.overlay = Overlay::HandOver(result);
                    msg
                }
                Event::GameOver { winner } => {
                    pause = DEMO_PAUSE;
                    self.overlay = Overlay::GameOver;
                    format!("{} won the game.", self.name(winner))
                }
            };
            self.log.push(msg);
        }
        if self.log.len() > 50 {
            self.log.drain(..self.log.len() - 50);
        }
        // the person's turn with nothing to do but pass: let them see why
        if self.your_turn() && self.game.legal() == vec![Action::Pass] {
            self.log.push("You cannot play.".into());
            pause = SHOW_PASS;
        }
        self.cursor = self.cursor.min(self.game.hands[0].len().saturating_sub(1));
        if let Some(m) = self.game.must_play.filter(|_| self.game.turn == 0) {
            self.cursor = self.game.hands[0].iter().position(|&t| t == m).unwrap_or(0);
        }
        self.wait_until = Instant::now() + pause;
    }

    /// Time passes: the computer moves, the person passes when they must,
    /// and the demo moves on by itself.
    pub fn tick(&mut self, now: Instant) {
        if self.screen != Screen::Table || now < self.wait_until {
            return;
        }
        match self.overlay {
            Overlay::HandOver(_) if self.demo => {
                self.overlay = Overlay::None;
                let events = self.game.next_hand();
                self.events(events);
            }
            Overlay::GameOver if self.demo => self.start(),
            Overlay::None if matches!(self.game.phase, Phase::Playing) => {
                match self.seats[self.game.turn] {
                    Some(strength) => {
                        let action = ai::choose(&self.game, strength, &mut self.rng);
                        self.act(action);
                    }
                    None if self.game.legal() == vec![Action::Pass] => self.act(Action::Pass),
                    None => {}
                }
            }
            _ => {}
        }
    }

    pub fn key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        match self.screen {
            Screen::Menu => self.menu_key(key.code),
            Screen::Table => self.table_key(key.code),
        }
    }

    fn menu_key(&mut self, code: KeyCode) {
        let change = |app: &mut App, step: i32| match app.menu_row {
            0 => app.setup.rules = (app.setup.rules as i32 + step).rem_euclid(3) as usize,
            1 => {
                app.setup.opponents =
                    ((app.setup.opponents as i32 - 1 + step).rem_euclid(3) + 1) as usize
            }
            2 => {
                let i = Strength::ALL
                    .iter()
                    .position(|&s| s == app.setup.strength)
                    .unwrap_or(1);
                app.setup.strength = Strength::ALL[(i as i32 + step).rem_euclid(3) as usize];
            }
            _ => {}
        };
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.menu_row = (self.menu_row + 3) % 4,
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.menu_row = (self.menu_row + 1) % 4
            }
            KeyCode::Left | KeyCode::Char('h') => change(self, -1),
            KeyCode::Right | KeyCode::Char('l') => change(self, 1),
            KeyCode::Enter | KeyCode::Char(' ') => self.start(),
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            _ => {}
        }
    }

    fn table_key(&mut self, code: KeyCode) {
        match &self.overlay {
            Overlay::Help => {
                self.overlay = Overlay::None;
                return;
            }
            Overlay::HandOver(_) => {
                if matches!(code, KeyCode::Enter | KeyCode::Char(' ')) {
                    self.overlay = Overlay::None;
                    let events = self.game.next_hand();
                    self.events(events);
                } else if matches!(code, KeyCode::Char('q') | KeyCode::Esc) {
                    self.screen = Screen::Menu;
                }
                return;
            }
            Overlay::GameOver => {
                match code {
                    KeyCode::Enter | KeyCode::Char(' ') => self.start(),
                    KeyCode::Char('q') | KeyCode::Esc => self.screen = Screen::Menu,
                    _ => {}
                }
                return;
            }
            Overlay::None => {}
        }
        match code {
            KeyCode::Char('?') | KeyCode::F(1) => self.overlay = Overlay::Help,
            KeyCode::Esc if self.choosing.is_some() => self.choosing = None,
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.demo {
                    self.quit = true;
                } else {
                    self.screen = Screen::Menu;
                }
            }
            _ if !self.your_turn() => {}
            KeyCode::Left | KeyCode::Char('h') => self.step(-1),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => self.step(1),
            KeyCode::Up | KeyCode::Down => self.step(1),
            KeyCode::Enter | KeyCode::Char(' ') => self.confirm(),
            KeyCode::Char('d') => self.draw(),
            _ => {}
        }
    }

    /// Move along the hand, or between ends while choosing one.
    fn step(&mut self, by: i32) {
        if let Some((ends, i)) = &mut self.choosing {
            *i = (*i as i32 + by).rem_euclid(ends.len() as i32) as usize;
            return;
        }
        let n = self.game.hands[0].len() as i32;
        if n > 0 && self.game.must_play.is_none() {
            self.cursor = (self.cursor as i32 + by).rem_euclid(n) as usize;
        }
    }

    fn confirm(&mut self) {
        if let Some((ends, i)) = self.choosing.take() {
            if let Some((tile, _)) = self.picked() {
                self.act(Action::Play(tile, ends[i]));
            }
            return;
        }
        match self.picked() {
            Some((tile, places)) if places.len() == 1 => self.act(Action::Play(tile, places[0])),
            Some((_, places)) => self.choosing = Some((places, 0)),
            None => {
                if self.game.legal() == vec![Action::Draw] {
                    self.draw();
                } else if self.your_turn() {
                    self.log.push("That tile does not fit.".into());
                }
            }
        }
    }

    fn draw(&mut self) {
        if self.your_turn() && self.game.legal().contains(&Action::Draw) {
            self.act(Action::Draw);
        } else if self.your_turn() {
            let why = match self.game.rules.drawing {
                rules::Drawing::None => "There is no drawing in the Block game.",
                rules::Drawing::Free { .. } => "The last two tiles in the boneyard stay there.",
                rules::Drawing::Forced if self.game.boneyard.is_empty() => "The boneyard is empty.",
                rules::Drawing::Forced => "You can play, so you may not draw.",
            };
            self.log.push(why.into());
        }
    }

    pub fn mouse(&mut self, m: MouseEvent) {
        if m.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        let (x, y) = (m.column as i32, m.row as i32);
        if self.screen == Screen::Menu {
            if let Some(&(.., row)) = self
                .hits
                .menu
                .iter()
                .find(|r| inside(x, y, (r.0, r.1, r.2, r.3)))
            {
                if row == 3 && self.menu_row == 3 {
                    self.start();
                } else if self.menu_row == row && row < 3 {
                    self.menu_key(KeyCode::Right);
                }
                self.menu_row = row;
            }
            return;
        }
        if !matches!(self.overlay, Overlay::None) {
            self.table_key(KeyCode::Enter);
            return;
        }
        if !self.your_turn() {
            return;
        }
        if let Some(&(.., arm)) = self
            .hits
            .ends
            .iter()
            .find(|r| inside(x, y, (r.0, r.1, r.2, r.3)))
        {
            if let Some((tile, _)) = self.picked() {
                self.act(Action::Play(tile, arm));
            }
            return;
        }
        if let Some(&(.., i)) = self
            .hits
            .hand
            .iter()
            .find(|r| inside(x, y, (r.0, r.1, r.2, r.3)))
        {
            if self.game.must_play.is_some() && self.cursor != i {
                return;
            }
            if self.cursor == i {
                self.confirm();
            } else {
                self.cursor = i;
                self.choosing = None;
            }
            return;
        }
        if self.hits.boneyard.is_some_and(|r| inside(x, y, r)) {
            self.draw();
        }
    }
}
