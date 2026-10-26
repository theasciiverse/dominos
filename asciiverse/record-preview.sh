#!/bin/sh
# Record the 100x30 catalog preview, asciiverse/dominos.cast, with hub's
# castrec.py: the computer playing every seat of All Fives (--demo), 45 s,
# from the first hand into the second, where the line turns corners.
set -e
cd "$(dirname "$0")/.."
here=$(pwd)
castrec=$(cd ../../hub/tools && pwd)/castrec.py
cargo build --release
tmp=$(mktemp -d)
cp target/release/dominos "$tmp/"
(cd "$tmp" && PATH="$tmp:$PATH" HOME="$tmp" XDG_DATA_HOME="$tmp/data" \
  python3 "$castrec" "$here/asciiverse/dominos.cast" . "w45;kq;w0.5;kq;w0.5" \
  -- dominos --demo --seed "${1:-1}")
rm -rf "$tmp"
