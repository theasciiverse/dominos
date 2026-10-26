//! Dominos: the Block game, the Draw game and All Fives in the terminal,
//! against the computer or with others on the local network.
//!
//! Options: `--seed N`, `--demo` (the computer plays every seat, game
//! after game), `--rules block|draw|fives`, `--quit-after SECONDS`. Log
//! with `RUST_LOG=debug dominos 2>game.log`.

mod ai;
mod app;
mod board;
mod draw;
mod game;
mod help;
mod layout;
mod net;
mod rng;
mod rules;
mod table;
mod tile;
mod ui;
mod view;

use app::App;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind,
};
use ratatui::crossterm::execute;
use std::io;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

fn main() -> io::Result<()> {
    if flag("--help") || flag("-h") {
        println!("dominos {}", env!("CARGO_PKG_VERSION"));
        println!("The Block game, the Draw game and All Fives in the terminal.\n");
        println!("  --seed N                  deal from seed N");
        println!("  --rules block|draw|fives  the game the menu starts on");
        println!("  --demo                    the computer plays every seat");
        println!("  --quit-after SECS         quit after SECS seconds");
        return Ok(());
    }
    env_logger::init();
    let seed = arg("--seed")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(1)
        });
    let quit_after = arg("--quit-after")
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64);
    let rules = match arg("--rules").as_deref() {
        Some("block") => 0,
        Some("draw") => 1,
        _ => 2,
    };
    let mut app = App::new(seed, rules, flag("--demo"));

    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let started = Instant::now();
    let result = (|| -> io::Result<()> {
        while !app.quit {
            terminal.draw(|f| app.hits = ui::render(&app, f.buffer_mut()))?;
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(k) if k.kind != KeyEventKind::Release => app.key(k),
                    Event::Mouse(m) => app.mouse(m),
                    _ => {}
                }
            }
            app.tick(Instant::now());
            if quit_after.is_some_and(|d| started.elapsed() >= d) {
                break;
            }
        }
        Ok(())
    })();
    execute!(io::stdout(), DisableMouseCapture)?;
    ratatui::restore();
    result
}
