# Notes for agents

This repository is for **making games** that run on Instance (a GameMaker 8.1
reimplementation). Instance itself is a separate repository at `../gm81-clean`; do not
change it from here.

- Read `docs/GUIDE.md` before writing GML: GM 8.1 has traps (no short-circuit
  evaluation; `&&`, `||` and `^^` share one precedence level, so bracket mixed
  conditions; scripts need exact argument counts).
- A game is a project folder in `games/<name>/`. Edit its `.gml` and `.toml` files
  directly. Do not generate whole projects with Rust or scripts. New games start as a
  copy of `games/_template/`.
- Each game in `games/` (except `_template`) is its own git repository. Commit game
  changes there, not in this repository.
- After every change, run `gmk-check games/<name>`. Then test headless with
  `instance-runner games/<name> --headless --no-read-fallback --save-dir <scratch dir> ...`.
  Never run tests without `--save-dir` and `--no-read-fallback`: the folder holds the
  player's real save files.
- Check visual changes by viewing screenshots (`--screenshot-at`), not just by
  reading code.
- Procedural assets go through a small tool that writes into the folder (example:
  Deepcore's `assets-src/`). Music sources live in
  `games/<name>/music-src/` and render with `gmk-music` into `sounds/`.
- Tools: `instance-runner`, `instance-ide`, `gmk-check`, `gmk-pack`, `gmk-unpack` come
  from `../gm81-clean/target/release`. `gmk-music` and `gmk-sounds` are built here.
