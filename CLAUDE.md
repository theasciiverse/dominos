# Dominos

A terminal dominoes game for the Asciiverse catalog (David, 2026-10-25).
Decided to build (David, 2026-10-25), beside chess-tui, termsuji (Go)
and the planned Backgammon. Not started: this folder and these notes
only, no code yet. Own git repo, branch `main`, private at
github.com/theasciiverse/dominos.

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
  2026-10-25)
- A graphical style of its own, clearly different from the Ancient
  Games (David, 2026-10-25)

## Open questions (David's to decide)

- Which games: the block and draw games, Muggins (All Fives), Mexican
  Train, Chinese dominoes, or several as one title with rule sets, as
  Mancala and Morris do in Ancient Games
- Stack: Rust + Ratatui as the rest of the catalog, unless decided
  otherwise
- Name (working name: Dominos)

Read the rules from real sources before building (pagat.com has many
domino games); nothing above about the games has been checked yet.
