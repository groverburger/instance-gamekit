# Instance gamekit

Everything for **making games** with [Instance](https://github.com/groverburger/instance),
the clean-room GameMaker 8.1 runner and IDE: example games, a starter template, the music
and sound tools, and the game-making guide.

Instance itself (runner, IDE, the `.gm81` file format, `gmk-check`) lives in its own
repository. This one only uses it.

## Setup

```sh
# Instance, checked out next to this repository
git clone git@github.com:groverburger/instance.git ../gm81-clean
(cd ../gm81-clean && cargo build --release)
export PATH="$PWD/../gm81-clean/target/release:$PATH"   # instance-runner, instance-ide, gmk-*

# this repository's tools: gmk-music, gmk-sounds
cargo build --release
scripts/fetch-soundfont.sh        # (or .ps1) General MIDI SoundFont for gmk-music
```

The Rust crates here depend on Instance's `instance-format` crate through a path
(`../gm81-clean`), set in `Cargo.toml`.

## Making a game

A game is a **project folder**: GML code in `.gml` files, sprites as PNGs, sounds as
WAVs, settings and metadata in small TOML files. Edit the files, check, run:

```sh
cp -r games/_template games/mygame
gmk-check games/mygame
instance-runner games/mygame                  # play it (runs the folder directly)
instance-ide games/mygame                     # or open it in the editor
gmk-pack games/mygame mygame.gm81             # one .gm81 file, for sharing
```

Read **[docs/GUIDE.md](docs/GUIDE.md)** first: the workflow, GML rules and gotchas,
runtime behaviour, headless testing, and game design advice.

## Layout

| Path | What |
|---|---|
| `docs/GUIDE.md` | the guide to making games with Instance |
| `games/_template/` | a small complete game to start from |
| `games/<name>/` | your games: each one is its own git repository, cloned or created here (ignored by this repository) |
| `crates/instance-music/` | `gmk-music` (text notation → MIDI/WAV) and `gmk-sounds` (loudness, clipping, loop seams); its README is the notation reference |
| `music_demos/` | music experiments (sources; renders are not committed) |
| `scripts/` | `fetch-soundfont` |

**Each game is its own git repository** inside `games/` (this repository ignores
everything there except the template). Start one with
`cp -r games/_template games/mygame && git -C games/mygame init`, or clone an existing
game into `games/`. The template's `.gitignore` keeps the save files a game writes into
its own folder when played (`*.sav`, `*.ini`, ...) out of git. The example games (Deepcore, Neon Swarm,
Hoverdrome, Skyhook, The Last Lanternlight) are such repositories.
