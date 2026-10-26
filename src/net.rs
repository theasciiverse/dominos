//! Play over the local network.
//!
//! The host runs the game (`table.rs`) and is the only one who knows every
//! hand: each player gets their own `View` and the events, with tiles
//! others draw hidden, and sends back only their actions, which the host
//! checks. Messages are JSON, one per line, over TCP. To find tables, a
//! player asks by UDP broadcast and every host answers with its `Advert`.

use crate::game::{Action, Event};
use crate::rules::Kind;
use crate::view::View;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Raised whenever the messages change.
pub const VERSION: u32 = 1;
/// The table's TCP port; if taken, any free one, which the advert names.
pub const PORT: u16 = 47520;
/// Where hosts listen for players looking for a table.
pub const FIND_PORT: u16 = 47521;
const ASK: &[u8] = b"DOMINOS?";
/// The longest message taken from anyone, in bytes.
const MAX_LINE: u64 = 1 << 20;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToHost {
    Hello {
        version: u32,
        name: String,
    },
    Act(Action),
    /// Ready for the next hand, or the next game.
    Ready,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToGuest {
    Welcome {
        seat: usize,
    },
    Lobby(Lobby),
    State {
        view: Box<View>,
        events: Vec<Event>,
        notes: Vec<String>,
    },
    Refused(String),
    Closed(String),
}

/// A table waiting for players.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lobby {
    pub kind: Kind,
    pub players: usize,
    pub teams: bool,
    /// The people seated so far; open seats go to the computer.
    pub names: Vec<Option<String>>,
}

/// A host's answer to a player looking for tables.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Advert {
    pub version: u32,
    pub port: u16,
    pub host: String,
    pub kind: Kind,
    pub players: usize,
    pub teams: bool,
    pub open: usize,
}

fn send_line<T: Serialize>(stream: &mut TcpStream, msg: &T) -> io::Result<()> {
    let mut line = serde_json::to_string(msg).map_err(io::Error::other)?;
    line.push('\n');
    stream.write_all(line.as_bytes())
}

/// Read messages until the stream ends or sends something not understood;
/// `deliver` gets each one, then `None` at the end, and returns false to
/// stop early.
fn read_messages<T: DeserializeOwned>(
    stream: TcpStream,
    mut deliver: impl FnMut(Option<T>) -> bool,
) {
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        match (&mut reader).take(MAX_LINE).read_line(&mut line) {
            Ok(0) | Err(_) => break,
            // too long, or cut off
            Ok(_) if !line.ends_with('\n') => break,
            Ok(_) => match serde_json::from_str(&line) {
                Ok(msg) => {
                    if !deliver(Some(msg)) {
                        return;
                    }
                }
                Err(e) => {
                    log::warn!("a message not understood: {e}");
                    break;
                }
            },
        }
    }
    deliver(None);
}

fn prepare(stream: &TcpStream) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))
}

/// What the host hears from its players.
#[derive(Debug)]
pub enum Incoming {
    Joined(usize),
    Message(usize, ToHost),
    Left(usize),
}

/// The host's side: accepts players and carries messages.
pub struct Server {
    pub port: u16,
    /// Whether players can find this table by broadcast (only one host per
    /// machine can); otherwise they type its address.
    pub findable: bool,
    rx: Receiver<Incoming>,
    streams: Arc<Mutex<HashMap<usize, TcpStream>>>,
    advert: Arc<Mutex<Option<Advert>>>,
    stop: Arc<AtomicBool>,
}

impl Server {
    pub fn start() -> io::Result<Server> {
        let listener =
            TcpListener::bind(("0.0.0.0", PORT)).or_else(|_| TcpListener::bind(("0.0.0.0", 0)))?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let (tx, rx) = channel();
        let streams: Arc<Mutex<HashMap<usize, TcpStream>>> = Arc::default();
        let advert: Arc<Mutex<Option<Advert>>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        {
            let (streams, stop) = (streams.clone(), stop.clone());
            thread::spawn(move || accept(listener, tx, streams, stop));
        }
        let findable = match UdpSocket::bind(("0.0.0.0", FIND_PORT)) {
            Ok(socket) => {
                let (advert, stop) = (advert.clone(), stop.clone());
                thread::spawn(move || answer(socket, advert, stop));
                true
            }
            Err(e) => {
                log::info!("no finding by broadcast: {e}");
                false
            }
        };
        Ok(Server {
            port,
            findable,
            rx,
            streams,
            advert,
            stop,
        })
    }

    pub fn poll(&self) -> Vec<Incoming> {
        self.rx.try_iter().collect()
    }

    pub fn send(&self, conn: usize, msg: &ToGuest) {
        let mut streams = self.streams.lock().expect("streams");
        if let Some(stream) = streams.get_mut(&conn)
            && let Err(e) = send_line(stream, msg)
        {
            // its reader will report it gone
            log::info!("player {conn} unreachable: {e}");
            let _ = stream.shutdown(Shutdown::Both);
        }
    }

    pub fn drop_conn(&self, conn: usize) {
        if let Some(stream) = self.streams.lock().expect("streams").remove(&conn) {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }

    /// What to answer players looking for tables; `None` to stay quiet.
    pub fn advertise(&self, advert: Option<Advert>) {
        *self.advert.lock().expect("advert") = advert;
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for stream in self.streams.lock().expect("streams").values() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

fn accept(
    listener: TcpListener,
    tx: Sender<Incoming>,
    streams: Arc<Mutex<HashMap<usize, TcpStream>>>,
    stop: Arc<AtomicBool>,
) {
    let mut next = 0;
    while !stop.load(Ordering::Relaxed) {
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => {
                log::warn!("accept: {e}");
                thread::sleep(Duration::from_millis(200));
                continue;
            }
        };
        next += 1;
        let conn = next;
        let Ok(writer) = prepare(&stream).and_then(|_| stream.try_clone()) else {
            continue;
        };
        streams.lock().expect("streams").insert(conn, writer);
        if tx.send(Incoming::Joined(conn)).is_err() {
            return;
        }
        let tx = tx.clone();
        thread::spawn(move || {
            read_messages(stream, |msg| match msg {
                Some(msg) => tx.send(Incoming::Message(conn, msg)).is_ok(),
                None => tx.send(Incoming::Left(conn)).is_ok(),
            })
        });
    }
}

fn answer(socket: UdpSocket, advert: Arc<Mutex<Option<Advert>>>, stop: Arc<AtomicBool>) {
    let _ = socket.set_read_timeout(Some(Duration::from_millis(200)));
    let mut buf = [0u8; 64];
    while !stop.load(Ordering::Relaxed) {
        let Ok((n, from)) = socket.recv_from(&mut buf) else {
            continue;
        };
        if &buf[..n] != ASK {
            continue;
        }
        let reply = advert.lock().expect("advert").clone();
        if let Some(advert) = reply
            && let Ok(json) = serde_json::to_vec(&advert)
        {
            let _ = socket.send_to(&json, from);
        }
    }
}

/// A table found on the network.
#[derive(Clone, Debug)]
pub struct Found {
    pub addr: SocketAddr,
    pub advert: Advert,
    seen: Instant,
}

/// Looks for tables: asks by broadcast every second, keeps the answers
/// heard in the last few.
pub struct Finder {
    socket: Option<UdpSocket>,
    asked: Option<Instant>,
    pub found: Vec<Found>,
}

impl Finder {
    pub fn new() -> Finder {
        let socket = UdpSocket::bind(("0.0.0.0", 0))
            .and_then(|s| {
                s.set_broadcast(true)?;
                s.set_nonblocking(true)?;
                Ok(s)
            })
            .map_err(|e| log::warn!("cannot look for tables: {e}"))
            .ok();
        Finder {
            socket,
            asked: None,
            found: Vec::new(),
        }
    }

    pub fn poll(&mut self, now: Instant) {
        let Some(socket) = &self.socket else { return };
        if self
            .asked
            .is_none_or(|t| now.duration_since(t) >= Duration::from_secs(1))
        {
            self.asked = Some(now);
            // the broadcast, and this machine, for a host here
            for to in [[255, 255, 255, 255], [127, 0, 0, 1]] {
                let _ = socket.send_to(ASK, SocketAddr::from((to, FIND_PORT)));
            }
        }
        let mut buf = [0u8; 2048];
        while let Ok((n, from)) = socket.recv_from(&mut buf) {
            let Ok(advert) = serde_json::from_slice::<Advert>(&buf[..n]) else {
                continue;
            };
            let addr = SocketAddr::new(from.ip(), advert.port);
            let same = |f: &Found| f.advert.port == advert.port && f.advert.host == advert.host;
            if from.ip().is_loopback() {
                // a host on this machine answers on both; keep the real address
                if self.found.iter().any(|f| same(f) && f.addr != addr) {
                    continue;
                }
            } else {
                self.found
                    .retain(|f| !(same(f) && f.addr.ip().is_loopback()));
            }
            self.found.retain(|f| f.addr != addr);
            self.found.push(Found {
                addr,
                advert,
                seen: now,
            });
        }
        self.found
            .retain(|f| now.duration_since(f.seen) < Duration::from_secs(4));
        self.found
            .sort_by(|a, b| a.advert.host.cmp(&b.advert.host).then(a.addr.cmp(&b.addr)));
    }
}

/// A player's connection to a host.
pub struct Connection {
    stream: TcpStream,
    rx: Receiver<Option<ToGuest>>,
}

impl Connection {
    pub fn connect(addr: SocketAddr, name: &str) -> io::Result<Connection> {
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3))?;
        prepare(&stream)?;
        let reader = stream.try_clone()?;
        let (tx, rx) = channel();
        thread::spawn(move || read_messages(reader, |msg| tx.send(msg).is_ok()));
        let mut conn = Connection { stream, rx };
        conn.send(&ToHost::Hello {
            version: VERSION,
            name: name.to_string(),
        })?;
        Ok(conn)
    }

    pub fn send(&mut self, msg: &ToHost) -> io::Result<()> {
        send_line(&mut self.stream, msg)
    }

    /// The messages in so far; `None` when the host has gone.
    pub fn poll(&self) -> Vec<Option<ToGuest>> {
        self.rx.try_iter().collect()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

/// An address typed in: a name or number, with or without `:port`.
pub fn resolve(text: &str) -> io::Result<SocketAddr> {
    let text = text.trim();
    let with_port = if text
        .rsplit_once(':')
        .is_some_and(|(_, p)| p.parse::<u16>().is_ok())
    {
        text.to_string()
    } else {
        format!("{text}:{PORT}")
    };
    with_port
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such address"))
}

/// This machine's address on the local network, to tell others: the one
/// a packet out would leave from (none is sent).
pub fn local_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    socket.connect(("192.0.2.1", 9)).ok()?;
    socket.local_addr().ok().map(|a| a.ip())
}
