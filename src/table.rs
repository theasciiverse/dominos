//! A game run here: the game itself and who sits at each seat (the person
//! at this keyboard, a computer player, or someone on the network), the
//! computer players' timing, and the next deal once everyone is ready.
//! When hosting, this is the one true game; players on the network get
//! their view of it after every change.

use crate::ai::{self, Strength};
use crate::game::{Action, Event, Game, Phase};
use crate::net::{Advert, Incoming, Lobby, Server, ToGuest, ToHost, VERSION};
use crate::rng::Rng;
use crate::rules;
use crate::view::View;
use std::io;
use std::time::{Duration, Instant};

const THINK: Duration = Duration::from_millis(800);
const DRAWING: Duration = Duration::from_millis(350);
const SHOW_PASS: Duration = Duration::from_millis(1400);
const DEMO_PAUSE: Duration = Duration::from_millis(3500);

/// The menu's choices for a game.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Index into `rules::ALL`.
    pub rules: usize,
    pub players: usize,
    pub teams: bool,
    pub strength: Strength,
}

impl Settings {
    pub fn teams(&self) -> bool {
        self.teams && self.players == 4
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sitter {
    /// The person at this keyboard.
    Here,
    Computer(Strength),
    /// Someone on the network, by connection.
    Remote(usize),
}

/// What changed: the game's events, and notes on the people at the table.
#[derive(Default)]
pub struct Update {
    pub events: Vec<Event>,
    pub notes: Vec<String>,
}

impl Update {
    fn is_empty(&self) -> bool {
        self.events.is_empty() && self.notes.is_empty()
    }
}

pub struct Table {
    pub game: Game,
    sitters: Vec<Sitter>,
    names: Vec<Option<String>>,
    server: Option<Server>,
    settings: Settings,
    demo: bool,
    ready: Vec<bool>,
    rng: Rng,
    seed: u64,
    wait_until: Instant,
}

impl Table {
    pub fn new(
        settings: Settings,
        sitters: Vec<Sitter>,
        names: Vec<Option<String>>,
        server: Option<Server>,
        seed: u64,
        demo: bool,
    ) -> (Table, Update) {
        let (game, events) = Game::new(
            rules::ALL[settings.rules],
            settings.players,
            settings.teams(),
            seed,
        );
        let mut table = Table {
            game,
            ready: vec![false; settings.players],
            sitters,
            names,
            server,
            settings,
            demo,
            rng: Rng::new(seed ^ 0x5EED),
            seed: seed.wrapping_add(1),
            wait_until: Instant::now(),
        };
        let update = table.publish(events, Vec::new());
        (table, update)
    }

    pub fn view(&self, seat: usize) -> View {
        let mut view = self.game.view(seat);
        view.names = self.names.clone();
        view.waiting = self.waiting();
        view
    }

    fn human(&self, seat: usize) -> bool {
        !matches!(self.sitters[seat], Sitter::Computer(_))
    }

    /// The people the next deal waits for.
    fn waiting(&self) -> Vec<usize> {
        if self.game_on() || self.demo {
            return Vec::new();
        }
        (0..self.sitters.len())
            .filter(|&s| self.human(s) && !self.ready[s])
            .collect()
    }

    fn game_on(&self) -> bool {
        matches!(self.game.phase, Phase::Playing)
    }

    /// `seat` plays `action`, if it is theirs to play.
    pub fn act(&mut self, seat: usize, action: Action) -> Result<Update, String> {
        if seat != self.game.turn {
            return Err("not your turn".into());
        }
        let events = self.game.apply(action)?;
        Ok(self.publish(events, Vec::new()))
    }

    /// `seat` is ready for the next hand or game.
    pub fn ready(&mut self, seat: usize) -> Update {
        if self.game_on() {
            return Update::default();
        }
        self.ready[seat] = true;
        if self.waiting().is_empty() {
            let events = self.advance();
            self.publish(events, Vec::new())
        } else {
            // let the others see who is ready
            self.publish(Vec::new(), Vec::new());
            Update::default()
        }
    }

    fn advance(&mut self) -> Vec<Event> {
        self.ready = vec![false; self.sitters.len()];
        match self.game.phase {
            Phase::HandOver(_) => self.game.next_hand(),
            Phase::GameOver { .. } => {
                let (game, events) = Game::new(
                    rules::ALL[self.settings.rules],
                    self.settings.players,
                    self.settings.teams(),
                    self.seed,
                );
                self.seed = self.seed.wrapping_add(1);
                self.game = game;
                events
            }
            Phase::Playing => Vec::new(),
        }
    }

    /// Time passes: messages from the network, the computer's moves, the
    /// passes people cannot avoid, the demo moving on.
    pub fn tick(&mut self, now: Instant) -> Update {
        let mut events = Vec::new();
        let mut notes = Vec::new();
        let mut changed = false;
        let incoming = self.server.as_ref().map(Server::poll).unwrap_or_default();
        for msg in incoming {
            changed |= self.hear(msg, &mut events, &mut notes);
        }
        if now >= self.wait_until && events.is_empty() {
            let seat = self.game.turn;
            match self.game.phase {
                Phase::Playing => match self.sitters[seat] {
                    Sitter::Computer(strength) => {
                        let action = ai::choose(&self.view(seat), strength, &mut self.rng);
                        events.extend(self.game.apply(action).unwrap_or_default());
                    }
                    _ if self.game.legal() == vec![Action::Pass] => {
                        events.extend(self.game.apply(Action::Pass).unwrap_or_default());
                    }
                    _ => {}
                },
                _ if self.demo => events.extend(self.advance()),
                _ => {}
            }
        }
        if events.is_empty() && notes.is_empty() && !changed {
            return Update::default();
        }
        self.publish(events, notes)
    }

    /// A message from the network; true if the others should hear of it.
    fn hear(&mut self, msg: Incoming, events: &mut Vec<Event>, notes: &mut Vec<String>) -> bool {
        let Some(server) = &self.server else {
            return false;
        };
        let seat_of = |conn| self.sitters.iter().position(|&s| s == Sitter::Remote(conn));
        match msg {
            Incoming::Joined(conn) => {
                server.send(conn, &ToGuest::Refused("The game has started.".into()));
                server.drop_conn(conn);
                false
            }
            Incoming::Message(conn, msg) => {
                let Some(seat) = seat_of(conn) else {
                    return false;
                };
                match msg {
                    ToHost::Act(action) if seat == self.game.turn => {
                        match self.game.apply(action) {
                            Ok(e) => events.extend(e),
                            Err(e) => server.send(conn, &ToGuest::Refused(e)),
                        }
                    }
                    ToHost::Ready if !self.game_on() => {
                        self.ready[seat] = true;
                        if self.waiting().is_empty() {
                            events.extend(self.advance());
                        }
                        return true;
                    }
                    _ => {}
                }
                false
            }
            Incoming::Left(conn) => {
                if let Some(seat) = seat_of(conn) {
                    let name = self.names[seat].take().unwrap_or_default();
                    self.sitters[seat] = Sitter::Computer(Strength::Normal);
                    notes.push(format!(
                        "{name} left the table; the computer plays their seat."
                    ));
                    if !self.game_on() && self.waiting().is_empty() {
                        events.extend(self.advance());
                    }
                }
                true
            }
        }
    }

    /// Tell everyone on the network what changed, and set the pause before
    /// the next computer move.
    fn publish(&mut self, events: Vec<Event>, notes: Vec<String>) -> Update {
        if let Some(server) = &self.server {
            for (seat, sitter) in self.sitters.iter().enumerate() {
                if let Sitter::Remote(conn) = *sitter {
                    let msg = ToGuest::State {
                        view: Box::new(self.view(seat)),
                        events: events.iter().map(|e| e.for_seat(seat)).collect(),
                        notes: notes.clone(),
                    };
                    server.send(conn, &msg);
                }
            }
        }
        let update = Update { events, notes };
        if !update.is_empty() {
            self.wait_until = Instant::now() + self.pause(&update.events);
        }
        update
    }

    fn pause(&self, events: &[Event]) -> Duration {
        if events
            .iter()
            .any(|e| matches!(e, Event::HandOver(_) | Event::GameOver { .. }))
        {
            return DEMO_PAUSE;
        }
        let seat = self.game.turn;
        if self.game_on() && self.human(seat) && self.game.legal() == vec![Action::Pass] {
            // let them see why they pass
            return SHOW_PASS;
        }
        match events.last() {
            Some(Event::Drew { .. }) => DRAWING,
            _ => THINK,
        }
    }

    /// Close the table, telling the players on the network.
    pub fn close(&mut self, why: &str) {
        if let Some(server) = &self.server {
            for sitter in &self.sitters {
                if let Sitter::Remote(conn) = *sitter {
                    server.send(conn, &ToGuest::Closed(why.into()));
                }
            }
        }
    }
}

/// A table on the network, waiting for players before the deal.
pub struct HostLobby {
    pub server: Server,
    pub settings: Settings,
    /// The people seated, by seat; the host is seat 0.
    pub names: Vec<Option<String>>,
    conns: Vec<Option<usize>>,
}

impl HostLobby {
    pub fn open(settings: Settings, host: &str) -> io::Result<HostLobby> {
        let server = Server::start()?;
        let mut names = vec![None; settings.players];
        names[0] = Some(host.to_string());
        let lobby = HostLobby {
            server,
            settings,
            names,
            conns: vec![None; settings.players],
        };
        lobby.tell();
        Ok(lobby)
    }

    pub fn lobby(&self) -> Lobby {
        Lobby {
            kind: rules::ALL[self.settings.rules].kind,
            players: self.settings.players,
            teams: self.settings.teams(),
            names: self.names.clone(),
        }
    }

    /// Show everyone who is seated, and answer those looking for tables.
    fn tell(&self) {
        let lobby = self.lobby();
        for conn in self.conns.iter().flatten() {
            self.server.send(*conn, &ToGuest::Lobby(lobby.clone()));
        }
        let open = self.names.iter().filter(|n| n.is_none()).count();
        self.server.advertise(Some(Advert {
            version: VERSION,
            port: self.server.port,
            host: self.names[0].clone().unwrap_or_default(),
            kind: lobby.kind,
            players: lobby.players,
            teams: lobby.teams,
            open,
        }));
    }

    /// Players coming and going; returns notes for the log.
    pub fn poll(&mut self) -> Vec<String> {
        let mut notes = Vec::new();
        for msg in self.server.poll() {
            match msg {
                Incoming::Joined(_) => {}
                Incoming::Message(conn, ToHost::Hello { version, name }) => {
                    let refuse = |why: &str| {
                        self.server.send(conn, &ToGuest::Refused(why.into()));
                        self.server.drop_conn(conn);
                    };
                    if version != VERSION {
                        refuse("This table runs another version of Dominos.");
                        continue;
                    }
                    if self.conns.contains(&Some(conn)) {
                        continue;
                    }
                    let Some(seat) = (1..self.names.len()).find(|&s| self.names[s].is_none())
                    else {
                        refuse("The table is full.");
                        continue;
                    };
                    let name = clean(&name);
                    self.names[seat] = Some(name.clone());
                    self.conns[seat] = Some(conn);
                    self.server.send(conn, &ToGuest::Welcome { seat });
                    notes.push(format!("{name} sat down."));
                    self.tell();
                }
                Incoming::Message(..) => {}
                Incoming::Left(conn) => {
                    if let Some(seat) = self.conns.iter().position(|&c| c == Some(conn)) {
                        let name = self.names[seat].take().unwrap_or_default();
                        self.conns[seat] = None;
                        notes.push(format!("{name} left."));
                        self.tell();
                    }
                }
            }
        }
        notes
    }

    /// Move whoever sits at `from` to `to`, changing places with whoever
    /// (or whichever open seat) is there. The host stays at seat 0. Those
    /// moved are told their new seats.
    pub fn swap(&mut self, from: usize, to: usize) {
        let n = self.names.len();
        if from == 0 || to == 0 || from >= n || to >= n || from == to {
            return;
        }
        self.names.swap(from, to);
        self.conns.swap(from, to);
        for seat in [from, to] {
            if let Some(conn) = self.conns[seat] {
                self.server.send(conn, &ToGuest::Welcome { seat });
            }
        }
        self.tell();
    }

    /// Deal: open seats go to the computer.
    pub fn start(self, seed: u64) -> (Table, Update) {
        self.server.advertise(None);
        let sitters = (0..self.settings.players)
            .map(|seat| match (seat, self.conns[seat]) {
                (0, _) => Sitter::Here,
                (_, Some(conn)) => Sitter::Remote(conn),
                _ => Sitter::Computer(self.settings.strength),
            })
            .collect();
        Table::new(
            self.settings,
            sitters,
            self.names,
            Some(self.server),
            seed,
            false,
        )
    }

    /// Close the lobby, telling those seated.
    pub fn close(&self, why: &str) {
        for conn in self.conns.iter().flatten() {
            self.server.send(*conn, &ToGuest::Closed(why.into()));
        }
    }
}

/// A name as shown at the table: printable, trimmed, at most 12 letters.
pub fn clean(name: &str) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).take(12).collect();
    let name = name.trim();
    if name.is_empty() {
        "Guest".into()
    } else {
        name.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Connection;
    use std::net::SocketAddr;
    use std::thread;

    /// The host moves a guest to the seat opposite, to be partners.
    #[test]
    fn the_host_moves_a_guest() {
        let settings = Settings {
            rules: 0,
            players: 4,
            teams: true,
            strength: Strength::Normal,
        };
        let mut lobby = HostLobby::open(settings, "Host").unwrap();
        let addr = SocketAddr::from(([127, 0, 0, 1], lobby.server.port));
        let guest = Connection::connect(addr, "Ann").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while lobby.names[1].is_none() {
            lobby.poll();
            assert!(Instant::now() < deadline, "the guest did not sit down");
            thread::sleep(Duration::from_millis(10));
        }
        lobby.swap(1, 2);
        lobby.swap(0, 2);
        assert_eq!(lobby.names[0].as_deref(), Some("Host"));
        assert_eq!(lobby.names[2].as_deref(), Some("Ann"));
        assert!(lobby.names[1].is_none());
        let mut seat = None;
        while seat != Some(2) {
            for msg in guest.poll() {
                if let Some(ToGuest::Welcome { seat: s }) = msg {
                    seat = Some(s);
                }
            }
            assert!(Instant::now() < deadline, "the guest was not told");
            thread::sleep(Duration::from_millis(10));
        }
        // the game deals Ann the seat she was moved to
        let (table, _) = lobby.start(1);
        assert!(matches!(table.sitters[2], Sitter::Remote(_)));
        assert!(matches!(table.sitters[1], Sitter::Computer(_)));
    }

    /// A host with two computer players and two people on the network,
    /// who play by the computer's choice from their own view, through to
    /// the end of a game.
    #[test]
    fn a_game_over_the_network() {
        let settings = Settings {
            rules: 2,
            players: 4,
            teams: true,
            strength: Strength::Normal,
        };
        let mut lobby = HostLobby::open(settings, "Host").unwrap();
        let addr = SocketAddr::from(([127, 0, 0, 1], lobby.server.port));
        let mut guests: Vec<Connection> = ["Ann", "Bo"]
            .iter()
            .map(|n| Connection::connect(addr, n).unwrap())
            .collect();
        let mut seats = [None; 2];
        let deadline = Instant::now() + Duration::from_secs(5);
        while lobby.names.iter().filter(|n| n.is_some()).count() < 3 {
            lobby.poll();
            assert!(Instant::now() < deadline, "the guests did not sit down");
            thread::sleep(Duration::from_millis(10));
        }
        let (mut table, _) = lobby.start(9);
        // the host's own seat plays by the computer too
        table.sitters[0] = Sitter::Computer(Strength::Normal);
        table.wait_until = Instant::now();
        let mut rng = Rng::new(1);
        let mut views: Vec<Option<View>> = vec![None; 2];
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            table.wait_until = Instant::now();
            table.tick(Instant::now());
            for (i, guest) in guests.iter_mut().enumerate() {
                for msg in guest.poll() {
                    match msg.expect("the host stays") {
                        ToGuest::Welcome { seat } => seats[i] = Some(seat),
                        ToGuest::State { view, events, .. } => {
                            // a guest never learns what others draw
                            for e in &events {
                                if let Event::Drew { who, tile } = e {
                                    assert_eq!(tile.is_some(), *who == view.seat);
                                }
                            }
                            views[i] = Some(*view);
                        }
                        ToGuest::Refused(why) => panic!("refused: {why}"),
                        _ => {}
                    }
                }
                let Some(view) = &views[i] else { continue };
                if view.my_turn() && view.legal() != vec![Action::Pass] {
                    let action = ai::choose(view, Strength::Normal, &mut rng);
                    guest.send(&ToHost::Act(action)).unwrap();
                    views[i] = None;
                }
            }
            if let Phase::GameOver { .. } = table.game.phase {
                break;
            }
            if let Phase::HandOver(_) = table.game.phase {
                for guest in &mut guests {
                    guest.send(&ToHost::Ready).unwrap();
                }
                table.ready(0);
            }
            assert!(Instant::now() < deadline, "the game did not end");
            thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(seats.iter().flatten().count(), 2);
    }
}
