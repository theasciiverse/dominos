# Dominos

A terminal dominoes game for the Asciiverse catalog (David, 2026-10-25),
beside chess-tui, termsuji (Go) and the planned Backgammon. Own git repo,
branch `main`, private at github.com/theasciiverse/dominos. Licence: not
chosen (David chooses case by case), so Cargo.toml names none.

**Status (2026-10-26):** version 0.1.0 playable: the Block game, the
Draw game and All Fives, you against one to three computer players at
three strengths, keys and mouse, help (F1 or ?), `--demo`.

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
  Ancient Games is amber on black, Backgammon brass, walnut and turquoise
- Games (David, 2026-10-26): Block, Draw and All Fives as one title with
  rule sets, chosen on the menu
- Stack (David, 2026-10-26): Rust + Ratatui 0.30, edition 2024
- Name (David, 2026-10-26): Dominos, binary `dominos`
- Players (David, 2026-10-26): one person against one to three computer
  players for now; **local network play soon**. Hence the engine is pure:
  `Game::apply(Action) -> Result<Vec<Event>>`, refusing anything not
  legal, dealing from a seeded RNG, the computer seeing only what its
  seat can see

## Rules and their sources

Read 2026-10-26 on pagat.com: Block (`pagat.com/tile/wdom/block.html`),
Draw (`.../draw.html`), All Fives (`pagat.com/domino/cross/all_fives.html`).
Main text used where the pages offer variations. Choices where the pages
leave a gap, to check with David:

- First lead by lot (all three pages: "draw for lead"), as a random seat
- Block and Draw scoring: the lowest count scores "the losers' total minus
  their own total", read as the others' pips summed, less the winner's
- All Fives: the page gives the blocked-hand scoring (others' pips to the
  nearest five) but not the scoring for playing out; the same is used
- All Fives deals 9, 7, 5 (pagat's main text; it lists other deals)
- All Fives four players: free-for-all here; pagat says it is usually
  played in partnerships (not built yet)

## Code

`src/`: `tile.rs`, `rules.rs` (rule sets as data), `board.rs` (the layout
as a root and four arms; the first double in All Fives becomes the root,
turning the line around it; the count of the ends), `game.rs` (deal,
legal actions, apply, hand scoring), `ai.rs`, `layout.rs` (screen
positions: arms run straight and turn clockwise at the table's edge, a
pinwheel; large pip tiles, else small numbered ones), `draw.rs`
(colours, tiles), `ui.rs`, `app.rs` (menu, input, timing), `help.rs`.

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

- Local network play: how players find each other, who deals
- Partnerships for four (All Fives, and the Block and Draw games' 100-point
  partnership game)
- More games later: Muggins, Fives and Threes, Mexican Train, Chinese
  dominoes
- Catalog entry, preview recording, licence
