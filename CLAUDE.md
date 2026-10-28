# Dominos

A terminal dominoes game for the Asciiverse catalog (David, 2026-10-25),
beside chess-tui, termsuji (Go) and Nard (backgammon). Own git repo,
branch `main`, private at github.com/theasciiverse/dominos. Licence: MIT
(David, 2026-10-27).

**Status (2026-10-26):** version 0.2.0 playable: the Block game, the
Draw game and All Fives, two to four players, partners at four, against
the computer at three strengths or with people on the local network,
keys and mouse, help (F1 or ?), `--demo`.

On the website mockup since 2026-10-26 with the NEW! banner: an entry in
`websites/Asciiverse/dev/mock-titles.toml` (original game, not in
`hub/catalog` yet; licence MIT), preview `asciiverse/dominos.cast`
(`asciiverse/record-preview.sh`, hub's `castrec.py`: `--demo --seed 1`,
45 s, still at 3 s) copied to the site's `casts/`, local Linux build
packaged by the site's `dev/build_catalog.py` into `dev-releases/`.

## Why build it

Checked 2026-10-25 (web search and GitHub search for dominoes with
terminal, TUI and ncurses): no good terminal dominoes game exists. The
nearest are the Python `dominoes` package (dominoes.readthedocs.io), a
line-based command-line game with computer players, and a few student
console projects (partaze/Python-For-Fun--Console-Dominoes,
mcesar96/Terminal-Tiles, aPeter1/dominoes-terminal and others), none
full-screen and none worth packaging.

## Settled

- Its own title, not part of the Ancient Games collection (David,
  2026-10-25), and shares no code with it
- A graphical style of its own (David, 2026-10-25): green baize, ivory
  tiles with black pips, crimson for what is chosen, gold for scores.
  Ancient Games is amber on black, Nard brass, walnut and turquoise
- Games (David, 2026-10-26): Block, Draw and All Fives as one title with
  rule sets, chosen on the menu
- Stack (David, 2026-10-26): Rust + Ratatui 0.30, edition 2024
- Name (David, 2026-10-26): Dominos, binary `dominos`
- Players (David, 2026-10-26): one person against one to three computer
  players, and local network play. The engine is pure:
  `Game::apply(Action) -> Result<Vec<Event>>`, refusing anything not
  legal, dealing from a seeded RNG
- Network play (David, 2026-10-26): **the host runs the game** and sends
  each player only their own `View` (their hand, the table, counts) and
  the events with others' drawn tiles hidden, so a modified client cannot
  see other hands; **tables are found by LAN broadcast**, with a typed
  address as the fallback; **empty seats are filled by the computer**
- Rule readings below kept as they are (David, 2026-10-26)
- Partners (2026-10-26, after pagat): four players only, sitting
  opposite; a menu option

## Rules and their sources

Read 2026-10-26 on pagat.com: Block (`pagat.com/tile/wdom/block.html`),
Draw (`.../draw.html`), All Fives (`pagat.com/domino/cross/all_fives.html`).
Main text used where the pages offer variations. Choices where the pages
leave a gap, kept by David (2026-10-26):

- First lead by lot (all three pages: "draw for lead"), as a random seat
- Block and Draw scoring: the lowest count scores "the losers' total minus
  their own total", read as the others' pips summed, less the winner's
- All Fives: the page gives the blocked-hand scoring (others' pips to the
  nearest five) but not the scoring for playing out; the same is used
- All Fives deals 9, 7, 5 (pagat's main text; it lists other deals)
- All Fives four players: free-for-all or partners, on the menu (pagat:
  usually partners)
- Partners in the Block and Draw games follow pagat's Partnership
  Dominoes (`pagat.com/tile/wdom/partnership.html`): partners opposite,
  a blocked hand goes to the side with the lower combined pips, which
  scores the losing side's pips, its own ignored; to 100 (the Block and
  Draw pages: "100 points in a two player or partnership game")
- Partners in All Fives: points made in play go to the side; a blocked
  hand goes to the side with fewer combined pips (pagat: "the player or
  team with fewest points"), scoring the opponents' pips to the nearest
  five; to 200

## Code

`src/`: `tile.rs`, `rules.rs` (rule sets as data), `board.rs` (the layout
as a root and four arms; the first double in All Fives becomes the root,
turning the line around it; the count of the ends), `game.rs` (deal,
legal actions, apply, hand scoring by side), `view.rs` (one seat's view:
the screen, the computer and the network all use it), `ai.rs` (chooses
from a `View`), `table.rs` (a game run here: who sits where, the
computer's timing, the next deal once every person is ready; the host's
lobby), `net.rs` (JSON lines over TCP port 47520, or any free port;
finding tables by UDP broadcast to port 47521, also asking 127.0.0.1 for
a host on the same machine; messages capped at 1 MB), `layout.rs`
(screen positions: arms run straight and turn clockwise at the table's
edge, a pinwheel; large pip tiles, else small numbered ones), `draw.rs`
(colours, tiles), `ui.rs`, `app.rs` (screens, input), `help.rs`.

Network behaviour: players who join take the open seats in order, and
the host can move them in the lobby (↑↓ a seat, ←→ move whoever sits
there; David, 2026-10-26), say to make two people partners; the host
keeps seat 1; a
started game is no longer advertised and refuses newcomers; a player who
leaves is replaced by the computer (normal); the next hand waits for
every person to press Enter; the host leaving closes the table for all.
`table::tests::a_game_over_the_network` plays a whole partners game over
loopback with two guests and checks no guest sees another's draw.

The large tiles fit while the layout is a line; a spinner's top and
bottom arms need about 24 rows of table, so on 100x30 the layout uses
the small numbered tiles once they start.

Computer strengths, measured 2026-10-26 (`cargo test --release --
--ignored --nocapture strengths`, 400 two-player games each): Block:
normal beats easy 371, hard beats easy 390, hard beats normal 278. Draw:
254, 344, 286. All Fives: 391, 400, 341. Hard in All Fives weighs the
next player's likely reply from the tiles it cannot see and the numbers
that player has shown they lack.

Options: `--seed N`, `--demo`, `--rules block|draw|fives`,
`--quit-after SECONDS`; `RUST_LOG=debug dominos 2>game.log`.

## Open questions (David's to decide)

- Play over the internet, beyond the local network
- More games later: Muggins, Fives and Threes, Mexican Train, Chinese
  dominoes
