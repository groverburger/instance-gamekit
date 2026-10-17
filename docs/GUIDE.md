# Making games with Instance

This guide is for anyone, usually an AI agent, who **makes or edits a game that runs on
Instance**. Instance is a reimplementation of GameMaker 8.1. Games target Instance and
the full GameMaker 8.1 feature set. Instance has no editions, so data structures,
surfaces, particles, blend modes, 3D and extensions all work.

The guide covers the workflow (project folders and the tools), the rules of the GML
language and the runtime that catch people out, sound and music, game design, and
automated testing. The file format and runtime specifications live in the Instance
repository (`specs/`). You only need them if this guide does not answer a question.

Paths are relative to the root of this repository (the "gamekit"). "GM" means
GameMaker 8.1.

## Contents

1. [Setup](#1-setup)
2. [The workflow](#2-the-workflow)
3. [The project folder](#3-the-project-folder)
4. [Top gotchas](#4-top-gotchas)
5. [The GameMaker 8.1 model in brief](#5-the-gamemaker-81-model-in-brief)
6. [Writing GML](#6-writing-gml)
7. [Runtime rules](#7-runtime-rules)
8. [Art](#8-art)
9. [Sound and music](#9-sound-and-music)
10. [Game design](#10-game-design)
11. [Testing](#11-testing)
12. [Where to look next](#12-where-to-look-next)

---

## 1. Setup

There are two repositories, side by side:

| Repository | Contents | Tools built there |
|---|---|---|
| Instance (`../gm81-clean`) | the GameMaker 8.1 reimplementation | `instance-runner`, `instance-ide`, `gmk-check`, `gmk-pack`, `gmk-unpack`, `gmk-info` |
| gamekit (this one) | the template, the art and music tools, this guide | `gmk-art`, `gmk-music`, `gmk-sounds` |

Build both:

```sh
(cd ../gm81-clean && cargo build --release)   # Instance tools -> ../gm81-clean/target/release
cargo build --release                         # gmk-art, gmk-music, gmk-sounds -> target/release
```

The gamekit's `Cargo.toml` expects the Instance checkout at `../gm81-clean`, because
the tools use its `instance-format` crate. Then put both `target/release`
directories on `PATH`:

```sh
export PATH="$PWD/../gm81-clean/target/release:$PWD/target/release:$PATH"      # bash
```
```powershell
$env:PATH = "$PWD\..\gm81-clean\target\release;$PWD\target\release;" + $env:PATH   # PowerShell
```

Alternatively, call the tools by path. Every command in this guide uses the bare tool
name and runs from the gamekit root.

**SoundFont.** MIDI and General MIDI instruments need a SoundFont. Run
`scripts/fetch-soundfont.sh` (or `.ps1`) once to download two into `soundfonts/`:

* `default.sf2` is the Arachno SoundFont, the default. Its author can't recommend
  commercial use, because it contains samples by other authors, some of them unknown.
* `GeneralUser-GS.sf2` is free for commercial use too. Render music for a game you sell
  with `--soundfont soundfonts/GeneralUser-GS.sf2`.

`gmk-music` finds `default.sf2` there when run from the gamekit root. For the runner and
for `--audio-out`, set `INSTANCE_SOUNDFONT` to the SoundFont's absolute path. Tracks that
use only the built-in synthesiser (`synth=`) need no SoundFont.

---

## 2. The workflow

**A game is a project folder** under `games/<name>/`. GML lives in `.gml` files, sprites
are PNGs, sounds are WAV files and metadata is in small TOML files (layout in §3). You
edit these files directly. Every tool reads the folder.

```sh
# edit files in games/mygame/ ...
gmk-check games/mygame                      # static check: compile errors, bad references, ...
gmk-art build games/mygame                  # art-src/*.art -> sprites and backgrounds (§8)
instance-runner games/mygame                # play it (windowed)
instance-runner games/mygame --headless ... # automated test (§11)
gmk-pack games/mygame mygame.gm81           # only to ship a single file
```

* **Start a new game** by copying the template:
  `cp -r games/_template games/mygame`. The template is a minimal starter with a title
  room, a game room, a controller object and pause. Then, in `project.toml`, give the
  game its own `game_id` (any other integer) and `guid` (32 hex digits). Set the window
  caption: `caption` in each `rooms/*/room.toml` and `window_caption` in
  `[game_information]`.
* **Edit a `.gm81`/`.gmk` from elsewhere**: `gmk-unpack game.gm81 games/mygame`, then
  work on the folder. `gmk-unpack` refuses a non-empty target. `--force` refreshes an
  existing folder in place, rewriting only changed files and leaving `.git`, notes and
  other extra files alone.
* **The IDE** opens folders too. `instance-ide games/mygame` saves back into the
  folder. It is the comfortable way to add resources that are fiddly by hand, such as
  new sprites with masks, rooms with many placed instances, or tiles. F5 runs the game.
* **Ship**: `gmk-pack games/mygame mygame.gm81` gives one file that Instance (and
  GameMaker 8.1) opens. The folder stays the source of truth. Don't edit the packed
  file.
* **Art** sources live in `games/<name>/art-src/*.art`, sprites and backgrounds as
  text (pixel grids, shapes, ramps, frames). `gmk-art build` renders them into the
  folder; `gmk-art sheet` and `gmk-art check` show and measure the result (§8).
* **Sound effects** are short `gmk-music` sources too: one-shots that use the built-in
  synthesiser, rendered into `sounds/` (§9.2).
* **Other generated assets**: anything the text tools cannot express comes from a small
  tool that writes into the folder. It does not generate the whole project. For
  example, Deepcore's `assets-src/` (crate `deepcore-assets`, older than §9.2)
  synthesises its sound effects in Rust. `cargo run --release --manifest-path games/deepcore/assets-src/Cargo.toml`
  reads `games/deepcore`, replaces the generated sounds by name, and writes back only
  the files that changed. Code, rooms and objects are never
  touched. The resources must already exist in the folder. A generator is a
  stand-alone crate inside the game's own repository (its `Cargo.toml` has an empty
  `[workspace]` table).
* **Music** sources live in `games/<name>/music-src/*.txt`. `gmk-music` renders them
  straight into `games/<name>/sounds/` (§9).

Do not build games by writing Rust (or other) programs that emit a whole project. The
folder is easier to read, diff, check and fix, and it is what the IDE and the tools
share.

**Each game is its own git repository** in `games/<name>/`; this repository ignores
everything in `games/` except the template. A new game: copy the template (it brings a
`.gitignore` for save files), then `git -C games/mygame init`. The example games
(Deepcore, a mining game with procedural art and adaptive music; Neon Swarm;
Hoverdrome; Skyhook; The Last Lanternlight) are separate repositories cloned into
`games/`.

---

## 3. The project folder

The full specification is `docs/project-folder.md` in the Instance repository. Below is
what you need day to day.

### 3.1 Layout

```text
games/mygame/
  project.toml             game id, guid, [settings], constants, room_order, [slots]
  game_information.rtf     F1 help text
  resource_tree.txt        the IDE's resource tree (folders); cosmetic
  settings/icon.ico
  sprites/spr_player/      sprite.toml + 0.png, 1.png, ... (one PNG per frame)
  backgrounds/bg_sky.toml + bg_sky.png
  sounds/snd_jump.toml + snd_jump.wav
  scripts/scr_move.gml + scr_move.toml
  fonts/fnt_hud.toml
  paths/path_patrol.toml
  timelines/tl_intro/      timeline.toml + step_<n>.gml per moment
  objects/obj_player/      object.toml + one file per event (create.gml, step.gml, ...)
  rooms/rm_level1/         room.toml, creation.gml, instances/<id>.gml
  triggers/<name>.toml + <name>.gml
  music-src/*.txt          (gamekit convention) gmk-music sources
```

* **References are by name**: `sprite = "spr_player"`, `parent = "obj_enemy"`,
  `mask_sprite = "spr_mask"`, `object = "obj_wall"` (room instances),
  `background = "bg_tiles"`, `room_order = ["rm_title", "rm_level1"]`. A missing key
  means "none".
* **Every resource has a slot `index`** (its id in GML) and a `name`. The `name` in
  the metadata counts. The file name is cosmetic.
* **Text**: write `.gml` with LF line breaks. Packing converts them to CR LF, which is
  GameMaker's convention. Text must be representable in Windows-1252: no emoji, no
  CJK. Typographic quotes and accented letters are fine. The packer and `gmk-check`
  report the file and line of an offending character.
* **Settings** are in `[settings]` of `project.toml`. The ones that matter most:
  `argument_errors` (exact script argument counts, keep `true`),
  `treat_uninitialized_as_zero` (keep `false`), `display_errors` (keep `true`),
  `write_errors_to_log`, `let_esc_end_game`, `let_f5_save_f6_load` (turn off in real
  games), `let_f4_fullscreen`, `display_cursor`, `scaling`.
* **Constants** are `constants = [{ name = "TILE", value = "32" }, ...]` in
  `project.toml`. The value is a GML expression.

### 3.2 Event file names

An event that is exactly one Execute Code action is a plain `<stem>.gml`, the normal
case. Anything else (several actions, drag-and-drop actions, "applies to") is
`<stem>.actions.toml`. Write code events as `.gml`.

| Event | File stem |
|---|---|
| Create, Destroy, Draw | `create`, `destroy`, `draw` |
| Alarm 0–11 | `alarm_0` … `alarm_11` |
| Step, Begin Step, End Step | `step`, `step_begin`, `step_end` |
| Collision with an object | `collision_<object name>` |
| Keyboard (held) / Key Press / Key Release | `keyboard_<key>`, `keypress_<key>`, `keyrelease_<key>` |
| Mouse | `mouse_left_button`, `mouse_left_pressed`, `mouse_left_released`, `mouse_right_pressed`, `mouse_no_button`, `mouse_enter`, `mouse_leave`, `mouse_global_left_pressed`, `mouse_wheel_up`, `mouse_wheel_down`, … |
| Other | `other_outside_room`, `other_intersect_boundary`, `other_game_start`, `other_game_end`, `other_room_start`, `other_room_end`, `other_no_more_lives`, `other_no_more_health`, `other_animation_end`, `other_end_of_path`, `other_close_button`, `other_user_0` … `other_user_15`, `other_outside_view_0`, `other_boundary_view_0`, … |
| Trigger | `trigger_<trigger name>` |

Keys are `vk_*` names (`keypress_vk_space`, `keyboard_vk_left`, `vk_enter`,
`vk_escape`, `vk_f1`, `vk_lshift`, …) or a letter or digit (`keyboard_A`,
`keypress_7`). Many games read the keyboard in Step with `keyboard_check(vk_left)` and
`keyboard_check_pressed(ord('Z'))` instead of using key events.

The `events` list in `object.toml` gives the order of the events in the IDE. A new event
file that is not listed is still picked up. Order matters only *within* an event's
action list. Execution order between objects is covered in §7.1.

### 3.3 Adding resources

A resource without an `index` gets the **next free slot** (after the highest existing
index and the count in `[slots]`), plus a leaf in the resource tree. You therefore never
need to edit `[slots]`. It only records the total so that trailing deleted slots
survive. If you do give an `index`, it must not collide with another resource's index;
the packer reports collisions. Missing metadata keys take default values.

| To add | Create | Notes |
|---|---|---|
| Script | `scripts/scr_name.gml` | The `.toml` (`index`, `name`) is optional. Without it the script is named after the file. |
| Event | `objects/obj_x/<stem>.gml` | File stems as in §3.2. |
| Object | `objects/obj_new/` with event files, and optionally `object.toml` | Keys: `sprite`, `solid`, `visible` (default true), `depth`, `persistent`, `parent`, `mask_sprite`. A new object gets the highest index, so it steps **last** (§7.1). |
| Sprite | `sprites/spr_new/0.png`, `1.png`, …, and optionally `sprite.toml` | Keys: `origin_x`, `origin_y`, `collision_shape` (1 rectangle, the default; 0 precise), `bbox_mode` (0 auto = tight box of opaque pixels, 1 full image, 2 manual with `bbox_left/right/top/bottom`). |
| Background | `backgrounds/bg_new.png`, and optionally `bg_new.toml` | Keys for tile sets: `use_as_tileset`, `tile_width`, `tile_height`. |
| Sound | `sounds/snd_new.wav` **and** `sounds/snd_new.toml` | A sound needs its TOML. Copy an existing one and change `name`, `data`, `file_name`. Set `volume = 1.0` and `file_type = ".wav"` explicitly: a missing `volume` is 0. `kind`: 0 for effects and WAV music, 1 for MIDI background music. |
| Room | `rooms/rm_new/room.toml` (+ `creation.gml`) | Add it to `room_order` in `project.toml`. Its first entry is the start room, and `room_goto_next()` follows the order. `room_goto(rm_new)` works either way. |
| Placed instance | an entry `{ id = 100010, object = "obj_x", x = 64, y = 32 }` in `instances` of `room.toml`; optional creation code in `rooms/<room>/instances/<id>.gml` | Ids are > 100000, unique across **all** rooms. Raise `last_instance_id` in `project.toml` to at least the highest id. Tiles use `last_tile_id` the same way. |
| Font | `fonts/fnt_new.toml` | Keys: `font_name`, `size`, `bold`, `italic`, `range_start = 32`, `range_end = 127`. |
| Time line | `timelines/tl_new/step_<n>.gml` per moment | |

**Deleting** a resource: delete its files and remove its line from
`resource_tree.txt`. Its slot becomes a deleted slot, and the ids of other resources do
not change. **Renaming**: change `name` and every reference. `gmk-check` and `gmk-pack`
report each dangling reference with its file and key, for example
``objects/obj_player/object.toml: key `sprite`: unknown sprite "spr_plyer"``.

---

## 4. Top gotchas

Read these first. Each one has caused real bugs.

| # | Rule | Details |
|---|---|---|
| 1 | **No short-circuit evaluation.** | `if (instance_exists(o) && o.hp > 0)` still evaluates `o.hp` and fails when `o` is gone. Nest the `if`s. §6.3 |
| 2 | **`&&`, `\|\|` and `^^` share ONE precedence level**, evaluated left to right. | `a \|\| b && c` means `(a \|\| b) && c`. Always bracket mixed conditions: `a \|\| (b && c)`. §6.2 |
| 3 | **Script argument counts must be exact.** | A call must pass exactly `1 + highest argumentN the script uses`. Otherwise you get "Illegal argument count". §6.6 |
| 4 | **Reading a variable that was never assigned is a runtime error.** This includes global array elements. | Initialise every instance variable in Create and every global (including every array element you will read) in the controller's Create. §6.4 |
| 5 | **No `++`, `--`, `?:`, `%`, `1e3`, `0xFF`, string escapes, array literals, `var a = 1`.** | Use `i += 1`, `mod`, `$FF`, `chr(34)`, and `#` for a newline in `draw_text`. §6.1 |
| 6 | **Constant folding errors are compile errors**: `'a' + 1`, `1 / 0`. | A compile error in a script stops the game at startup. `gmk-check` finds these. §6.1 |
| 7 | A code piece whose **first token is `{`** must be exactly one block. | Anything after the matching `}` is "Program ends before end of the code." §6.1 |
| 8 | `var` locals live for **one code piece** only, and `var` cannot use a built-in name (`x`, `speed`, …). | §6.4 |
| 9 | A resource name is a constant, not a variable. | `global.obj_score` fails when an object `obj_score` exists. Prefix every resource (`obj_`, `spr_`, …). A script named like a built-in (`abs`) **replaces** it. §6.5 |
| 10 | Truthiness: a real is true iff **≥ 0.5**. A string used as a condition is a runtime error. | `if (0.4)` is false. §6.3 |
| 11 | `string(x)` prints non-integers with exactly 2 decimals. | `string(0.125)` = `"0.13"`. Use `string_format`. §6.7 |
| 12 | `room_goto()` lets the current code piece finish, then **skips the rest of the event and the step**. | Put it last. §7.3 |
| 13 | Room creation code runs **after** every Create event. Instance creation code runs **before** that instance's Create. | Read room settings in Room Start. Never overwrite instance-creation variables in Create. §7.3, §10.2 |
| 14 | Step, Begin Step and End Step run **object by object, in object index order**, not in instance creation order. | A controller that must run first needs the lowest object index. §7.1 |
| 15 | Alarms only count down if the object has (or inherits) that Alarm event. `alarm[i] = 0` never fires. | §7.2 |
| 16 | Collisions do nothing unless one of the objects has a collision event. With a handler, a **solid** partner puts both back to their previous positions. | §7.4 |
| 17 | An object **with** a Draw event does not draw its sprite. | Call `draw_self()` or `draw_sprite(...)`. §7.6 |
| 18 | There is **no Draw GUI event**. | Draw the HUD in Draw at `view_xview[view_current] + dx`. §7.6 |
| 19 | Draw state (colour, alpha, font, alignment) is global and persists. | Set it before every drawing block. §7.6 |
| 20 | Arrays are not values. | `b = a` copies `a[0]`. Scripts cannot take or return arrays. §6.4 |
| 21 | With **"Esc ends the game" off, also turn "treat the close button as Esc" off** (`let_esc_end_game` / `treat_close_as_esc` in `[settings]`). | Otherwise the window's close button only presses Esc (often "pause") and the window cannot be closed. With it off, the close button ends the game, or runs the Close Button event (`other_close_button`) if an object has one, which must then call `game_end()`. The template does this. §7.3 |

---

## 5. The GameMaker 8.1 model in brief

| Resource | What it is |
|---|---|
| Sprite | animated image (frames), origin, collision mask settings |
| Sound | an embedded WAV, MIDI or MP3 file plus its kind (§7.7) |
| Background | one image, also usable as a tile set; rooms show up to 8 background/foreground layers |
| Path | a list of points for `path_start` (straight or smooth, open or closed) |
| Script | a named GML function, 0–16 arguments |
| Font | system font name, size and character range, for `draw_set_font` |
| Time line | a list of (step, code) moments attached to an instance |
| Object | sprite, solid, visible, depth, persistent, parent, mask, plus **events with code** |
| Room | size, speed, colour, creation code, 8 backgrounds, 8 views, **placed instances**, tiles |
| Also | Global Game Settings, constants, triggers, game information, room order |

* **Resource ids are slot indices** (0, 1, 2, …). In GML a resource name is a constant
  that holds its index, and `-1` (or `noone` for instances) means none.
* **Objects** have events: Create, Destroy, Alarm 0–11, Begin Step/Step/End Step,
  Collision with object X, Keyboard/Key Press/Key Release, Mouse, Other (room start,
  user events, …), Draw and Trigger. Write every event as GML code.
* **Instances** are runtime copies of objects. Instances placed in a room have fixed
  ids (> 100000). Instances created at run time get new ids.
* **Parents**: an object inherits every event it does not define from its parent chain.
  `with (parent)`, collision events against the parent and
  `instance_number(parent)` include descendants. `event_inherited()` runs the parent's
  version of the current event.
* **Rooms** play in `room_order`. Its first entry is the start room.

---

## 6. Writing GML

### 6.1 Syntax: what exists and what doesn't

| Exists | Does **not** exist |
|---|---|
| `{ }` / `begin end`, optional `;`, `if … then … else`, `repeat (n)`, `while`, `do … until`, `for (init; cond; step)`, `switch`/`case`/`default` (falls through; strings allowed), `break`, `continue`, `exit`, `return expr`, `with (expr)`, `var a, b;`, `globalvar a;` | `++` / `--` (`i++` is "Assignment operator expected."), `?:`, `%`, `%=`, `<<=`, `>>=`, `mod=` |
| `= := += -= *= /= \|= &= ^=` | `var a = 1;` (no initialiser), `return;` without a value (use `exit`) |
| `and or xor not div mod`, `&& \|\| ^^ !`, `& \| ^ ~ << >>`, `<>` (= `!=`) | exponent literals (`1e3`), `0xFF` (write `$FF`) |
| strings in `'…'` or `"…"`, which may span lines | escape sequences (`\n` and `\"` are literal text): use `chr(34)`, `chr(39)`, and `#` for a line break in `draw_text` (`\#` is a literal `#`) |
| 1-D and 2-D arrays `a[i]`, `a[i, j]` | array literals, arrays as values, accessors (`[\| ]`, `[? ]`, `[# ]`, `[@ ]`), structs, methods, `enum`, `#macro` |
| `a.b.c`, `(expr).var`, `f(x).var` | indexing a call result (`f()[0]`), `100032.speed` (write `(100032).speed`) |

* Identifiers are case-sensitive. `true false pi self other all noone global local`
  are constants.
* **`=` inside an expression is a comparison**: `a = b = c` assigns `b == c` to `a`, and
  `if (x = 3)` compares.
* **Constant folding**: operations on literals are evaluated at compile time, so
  `'a' + 1` ("Wrong type of arguments to +.") and `1 / 0` ("Division by 0.") are
  **compile errors**. A compile error in a script stops the game at startup. In an
  event, it is fatal when the event first runs.
* **First token `{`**: that block is the whole program. Any token after its closing `}`
  is "Program ends before end of the code.". Either put everything in one outer block or
  start with something else, such as a comment.
* Calls need parentheses. Built-in functions have a fixed number of arguments (a wrong
  count is a compile error), except the variadic ones (`choose`, `min`, `max`, `mean`,
  `median`, `script_execute`, …). The maximum is 16 arguments.

### 6.2 Precedence (lowest to highest; all left-associative)

1. `&& || ^^` (`and or xor`): **one level**. `a || b && c` = `(a || b) && c`
2. `< <= == = != <> >= >`: one level. `3 > 2 > 1` is `(1) > 1` = 0
3. `| & ^`: one level. `a & b == c` = `(a & b) == c`
4. `<< >>`
5. `+ -`
6. `* / div mod`
7. unary `! - + ~`
8. `.`, `[ ]`, calls

**Bracket every condition that mixes `&&` and `||`.** This caused a real bug in
Deepcore's scanner: `if (t >= 11 && t <= 22 || (t == 23 || t == 24) && tier >= 5)` was
meant as "ore, or treasure at tier 5", but it parses as
`((t >= 11 && t <= 22) || (t == 23 || t == 24)) && tier >= 5`, so no ore was marked
below tier 5. `gmk-check` does not catch it; screenshots did.
Most other languages would parse it as intended. `gmk-check` does not warn about this.
When in doubt, parenthesise.

### 6.3 Values, truthiness, comparison, no short-circuit

* Values are **reals** (doubles) or **strings**. Booleans are 1 and 0, and ids,
  colours and resources are reals. Nothing converts implicitly: use `string(v)` and
  `real(s)`.
* Mixing types is a runtime error. `'a' + n` gives "Wrong type of arguments to +.",
  comparing a string with a real gives "Cannot compare arguments.", and a string used as
  a condition gives "Expression expected". `+=` on strings concatenates.
* **Truthiness**: a real is true iff `>= 0.5`. `0.5` is true, `0.49` is false and
  negative numbers are false.
* **Equality** has an absolute tolerance: reals are equal when `|a - b| < 1e-13`.
  `sin(pi) == 0` and `0.1 + 0.2 == 0.3` are true.
* **No short-circuit**: both operands of `&& || ^^` are always evaluated. Guard
  dependent reads with nested `if`s:
  ```gml
  if (instance_exists(target)) { if (target.hp > 0) { ... } }
  ```
  The same applies to array bounds: `if (i < n && list[i] > 0)` reads `list[i]` even
  when `i >= n`.
* Conversions to integers round **half to even**: `repeat (2.5)` runs twice,
  `round(2.5) = 2`, `a[1.5]` is `a[2]`, and `alarm[0] = 1.5` stores 2. `div` truncates.
  `mod` keeps the sign of the dividend and works on fractions. `/`, `div` and `mod` by
  0 are errors.

### 6.4 Variables

| Kind | Syntax | Lifetime and rules |
|---|---|---|
| instance | `hp = 3;` | Created on first assignment. Other code reads it as `inst.hp`. |
| local | `var i, n;` | Lasts **one code piece** (event code or script call). It is visible inside `with` bodies. It **cannot have a built-in name** (`var x;` is "Cannot redeclare a builtin variable."). |
| global | `global.level = 1;` or `globalvar level;` | Lasts the whole game. **Cleared by `game_restart()`**. |
| built-in | `x`, `speed`, `alarm[0]`, `room`, `score`, `view_xview[0]`, … | Fixed types. Assigning a string to a real built-in silently stores 0. Writing a read-only one (`id`, `instance_count`) is an error. |

* **Reading an unassigned variable** is a runtime error ("Unknown variable foo").
  Initialise every instance variable in Create and every global in the controller's
  Create. `variable_local_exists('name')` and `variable_global_exists('name')` test
  whether a variable exists.
* **Arrays**:
  * Each row has its own length. Elements below the highest index written in that row
    read 0. Anything beyond it, or in a row never written, is an error ("Unknown
    variable a or array index out of bounds"). **Initialise a global array completely
    before anything reads it**, typically with a loop in the controller's Create that
    writes the highest index first:
    ```gml
    global.best[LEVELS - 1] = 0;          // allocates 0..LEVELS-1, all reading 0
    for (i = 0; i < LEVELS; i += 1) global.best[i] = 0;
    ```
  * A plain variable and element `[0]` are the same storage: `a = 5` means
    `a[0] == 5`. A 1-D `a[i]` is row 0 of a 2-D array (`a[3]` ≡ `a[0, 3]`).
  * Index limits are `0 ≤ index < 32000` per dimension.
  * Arrays are not values: `b = a` copies `a[0]` only, and scripts cannot take or return
    arrays. Pass an instance id, use globals, or use a data structure (`ds_list`,
    `ds_grid`, `ds_map`).

### 6.5 Names and resolution

* Name lookup: keyword, then function call (built-in, **but a script with the same name
  wins**), then `var` local, then built-in variable, then constant (resource name, then
  user constant, then built-in constant), then instance or global variable.
* A resource name is a constant. You cannot assign to it or use it after a `.`:
  `global.obj_player = 1` is "Variable name expected.". **Prefix every resource**
  (`spr_`, `snd_`, `bg_`, `path_`, `scr_`, `fnt_`, `tl_`, `obj_`, `rm_`) and never name
  variables like them.
* Never name resources or variables like built-ins, keywords or constants (`score`,
  `health`, `self`, `c_red`, `abs`, `speed`).

### 6.6 Scripts and arguments

* Call scripts as `scr_name(a, b)` with up to 16 arguments. Inside, use
  `argument0..argument15`, `argument[i]` and `argument_count`. A script runs with the
  caller's `self` and `other`.
* With `argument_errors = true` (keep it on), a call must pass **exactly** `1 + highest
  argumentN used` arguments, or you get "Illegal argument count calling script …". A
  script that uses `argument[i]` accepts at least that many. If you change a script's
  arguments, update every call (`gmk-check` reports mismatches).
* Return values with `return expr;` and always return explicitly.
* `exit` leaves the current script or event code. It does not leave the event's other
  actions or the caller.

### 6.7 Library behaviours that surprise

* `string(x)`: integers print as integers, and other reals print with **2 decimals**
  (`string(2.675) = "2.67"`). Use `string_format(x, total, decimals)` to control the
  output.
* `real(s)` is strict: `"12abc"` gives an error and returns 0.
* Errors in **built-in functions** (`sqrt(-1)`, `sound_play(99)`) are reported, return
  0, and the code continues. Errors in **code** (operators, unknown variables, a missing
  instance) abandon the rest of the current code piece. The next action still runs.
* Strings are one byte per character (Windows-1252). `chr(n)` takes 0..255, and
  `string_length` counts bytes.
* `random(n)`, `irandom(n)` and `choose(…)` are seeded randomly at start. Call
  `random_set_seed(k)` for reproducible sequences, for example a level generated from a
  seed.
* `file_text_open_read` returns -1 for a missing file (no error). Check `file_exists`
  first.
* `show_message`, `get_string` and `show_question` are modal and pause the game. Avoid
  them in finished games and draw your own UI instead.
* `execute_string(code)` compiles at run time and is slow. Never use it every step.

### 6.8 Instances, `.` and `with`

| Owner | read `o.v` | write `o.v = …` |
|---|---|---|
| `self`, `other` | that instance | that instance |
| `all` | the first instance | all instances |
| `noone` | error | error |
| `global` | global variable | global variable |
| object index | the first instance of the object **or a descendant** | all of them (`obj.y += 8` reads the first and writes all) |
| instance id | that instance; error if it is gone | that instance; does nothing if it is gone |

`with (expr) stmt` runs `stmt` once per target, with `self` = the target and
`other` = the previous `self`. The target list is a **snapshot**, so instances created
inside the loop are not visited. Instances destroyed before their turn are skipped.
Descendants are included. `with (noone)` does nothing, `break` ends the loop, and `var`
locals stay visible.

`other` outside `with`: in a collision event it is the partner. In a Create event run by
`instance_create`, and in a Destroy event, it is the instance itself. `event_user` keeps
the caller's `other`.

`instance_destroy()` runs the Destroy event immediately. After that, `instance_exists`
is false, but the destroyed instance's running code can still read its own variables.

`instance_change(obj, perf)` behaves surprisingly. The code that called it keeps seeing
the **old** object, and its writes after the call are lost. Prefer destroying the
instance and creating a new one.

---

## 7. Runtime rules

### 7.1 Order of one step

1. Begin-step triggers, **Begin Step**
2. Time lines advance (their moments run)
3. **Alarms** (alarm 0 for all objects, then alarm 1, …)
4. **Keyboard** (held), then **Key Press**, then **Key Release**
5. **Mouse** events
6. Step triggers, **Step**
7. Motion: friction, gravity, paths, then `x += hspeed; y += vspeed`
8. Boundary events (Outside Room, Intersect Boundary, view variants)
9. **Collision** events
10. End-step triggers, **End Step**
11. Destroyed instances are removed
12. **Draw** (once per visible view), then `image_index` advances and **Animation End**
    fires

**Dispatch order**: Begin Step, Step, End Step and alarms run **object by object in
ascending object index**, and within an object in instance order. An instance created
during a phase runs in that phase only if its object has not been processed yet. Room
Start and Room End use instance (creation) order. Consequence: a controller that must
step before the player needs a **lower object index**. A new object without an `index`
gets the highest index (§3.3), so set `index` (or reorder) when the order matters.
Alternatively, do the dependent work in End Step.

* Animation: `image_index += image_speed` after Draw. When it reaches `image_number` it
  wraps and fires Animation End. A negative `image_speed` never wraps. Draw sees the
  value from before the advance.
* Outside Room fires **every step** while the bounding box is completely outside the
  room. Intersect Boundary fires every step while the box is not completely inside.
  Destroy or move the instance to stop it.
* No More Lives and No More Health fire **at the assignment** that makes
  `lives`/`health` ≤ 0.
* `keyboard_check_pressed` is true for one step. `keyboard_key_press(k)` (simulated
  input) takes effect in the next step.

### 7.2 Alarms

* There are 12 per instance, `alarm[0..11]`, each defaulting to -1.
* Each step, an alarm > 0 counts down by one. When it reaches 0 the event fires.
  `alarm[i] = 0` never fires, `alarm[0] = 1` fires next step, and
  `alarm[0] = room_speed` fires after one second.
* **Alarms only tick if the object has (or inherits) that Alarm event.**
* For a repeating timer, re-arm the alarm inside its own event: `alarm[0] = 30;`.

### 7.3 Rooms

* `room_goto(r)`, `room_goto_next()`, `room_restart()` and `room = r` let the **current
  code piece finish**. The rest of that event is skipped, and so is the rest of the step
  (no later instances' events, no End Step, no Draw). Then Room End runs for every
  instance and the new room is entered. Put `room_goto` last.
* **Entering a room**: first all placed instances are added. Then, for each placed
  instance, its **instance creation code** runs, followed by its **Create event**. Next
  comes **Game Start** (first room only), then the **room creation code**, then **Room
  Start** for every instance. One frame is drawn before the first Begin Step.
* Persistent instances survive room changes. A placed instance of a persistent object
  in a *different* room is a second copy, so place a persistent controller in exactly
  one room that is entered once (the title or a boot room).
* Persistent rooms keep their state. Re-entering one does not run its creation code or
  Create events again, but restored instances do get Room Start.
* `room_goto_next()` in the last room is an error. Check `room_next(room) != -1` first.
* `game_restart()` runs Room End and Game End, removes all instances without running
  Destroy, **clears globals**, and sets `score` to 0 and `lives` to -1. `health` keeps
  its value.
* At game start, `show_score` is 1, so the window caption shows "Score: 0". Set
  `show_score = false;` in the controller's Create unless you want that.

### 7.4 Collisions

* A collision event fires only for objects that **have** a collision event for the other
  object (or one of its ancestors). Without one, instances pass through each other,
  solid or not.
* Each colliding pair is handled once per step: the first instance's event, then the
  partner's event if it has one.
* If **either** instance is solid, both are moved back to their `xprevious/yprevious`
  before the events. Speed is not changed, so a blocked mover keeps pushing every step.
  Set `speed`/`hspeed` to 0 or use `move_contact_solid` in the event for clean stops.
* Collisions use the sprite's mask or `mask_index`, never what Draw draws. Invisible
  instances still collide. An instance without a sprite or mask never collides.
* Boxes touch only when they really overlap. A pixel belongs to an instance only if its
  centre lies inside the mask.
* `place_free(x, y)` tests only solid instances. `place_meeting(x, y, obj)` tests any
  instance of `obj`, and `instance_place` returns the first one it finds. For tile-based
  worlds, a grid lookup (an array or `ds_grid`) is much faster than thousands of wall
  instances.

### 7.5 Motion, paths, time lines

* `speed/direction` and `hspeed/vspeed` are coupled. Direction is in degrees, 0 = right,
  counter-clockwise. **y grows downward**, so 90 = up. `friction` reduces speed each
  step. `gravity` plus `gravity_direction` (270 = down) add to the velocity each step.
* Paths: `path_start(path, speed, endaction, absolute)`. The end actions are 0 stop,
  1 restart, 2 continue and 3 reverse. End of Path fires in every case.
* Time lines: set `timeline_index`, `timeline_position = 0` and
  `timeline_running = true`. Assigning the index alone does not start it.

### 7.6 Drawing and views

* Draw order: higher `depth` is drawn first (behind) and lower or negative depth last
  (in front). Equal depths keep the previous frame's order.
* An object **with** a Draw event does not draw its sprite automatically. Call
  `draw_self()` (which uses `image_xscale`, `image_angle`, `image_blend`, `image_alpha`)
  or `draw_sprite(sprite_index, image_index, x, y)`. Invisible instances get no Draw
  event, but they still step and collide.
* **Draw state is global** (`draw_set_color`, `draw_set_alpha`, `draw_set_font`,
  `draw_set_halign`/`valign`, blend mode) and persists between events and frames. Set
  it at the start of every drawing block and reset alpha and blend mode afterwards.
* Drawing outside the Draw event is wiped by the next frame.
* **Views**: with views enabled, Draw runs once per visible view, in room coordinates.
  There is no GUI layer. Draw the HUD from an object with very low depth (e.g. -10000)
  at view-relative positions:
  ```gml
  var vx, vy;
  vx = view_xview[view_current]; vy = view_yview[view_current];
  draw_set_color(c_white); draw_set_alpha(1);
  draw_text(vx + 8, vy + 8, 'Score: ' + string(score));
  ```
  Check `view_current` if several views are visible and the HUD should appear once.
* View following: set `follow_object` and the borders on the view in `room.toml`. A
  border of at least half the view keeps the target centred. Many games move
  `view_xview[0]` themselves in End Step for smoothing and clamping.
* Text: `#` is a line break in `draw_text`. With no font set, the font is Arial 12, so
  add Font resources for a consistent look.
* Colours: `make_color_rgb(r, g, b)` (= `r + 256*g + 65536*b`), `merge_color`, and
  constants like `c_white`.
* Off-screen rendering (`surface_*`), blend modes (`draw_set_blend_mode(bm_add)` for
  glows), particles (`part_*`), `draw_sprite_ext` and `draw_primitive_*` all work.
  Surfaces can be lost: check `surface_exists` before using one each frame.

### 7.7 Sound playback

| Kind | Use |
|---|---|
| 0 normal | Effects, and WAV music. Any number of overlapping copies. |
| 1 background music | MIDI music. One at a time: starting one stops the current one. |
| 2 3D | positional effects |
| 3 multimedia | MP3. One at a time, with no effects or volume control. |

`sound_play`, `sound_loop`, `sound_stop`, `sound_stop_all`, `sound_isplaying` and
`sound_volume(snd, 0..1)` control playback. Sounds continue across room changes, and
`game_restart` stops them. Sound data is stored uncompressed: 22050 Hz mono WAV is
about 2.6 MB per minute and 32000 Hz mono about 3.8 MB per minute, while MIDI is tiny.
For crossfades between music tracks, loop both as kind 0 WAVs and ramp `sound_volume`
over about one second (Deepcore's `obj_music` does this).

---

## 8. Art

`crates/instance-art` provides `gmk-art`: sprites and backgrounds written as text, a
contact sheet to look at them, and checks that measure what is hard to judge by eye.
The format reference is [`crates/instance-art/README.md`](../crates/instance-art/README.md).
The template's `art-src/sprites.art` is a short example.

### 8.1 `.art` files and `gmk-art build`

```text
# games/mygame/art-src/hero.art
ramp abcde #3a78d8          # five blues, dark to light, hue-shifted like painted shading
color k #101828

sprite spr_hero 16x16
origin center
rect 3 2 10 12 c
grid 0 5                    # pixel rows: the left eye
|.....k
mirror x                    # the right half
shade tl                    # light from the top left
outline k                   # once, at the end
```

* Small sprites (icons, characters, items): **pixel grids**, with `mirror` for
  symmetric shapes. Larger ones: **shapes** (`rect`, `circle`, `poly`, `line`), then
  `noise`, `speckle`, `gradient` for texture, then `shade` and `outline`.
* Colours come from **ramps** (`ramp 12345 #c82832`). Shading with neighbouring ramp
  keys, or `k+1` / `k-1`, keeps a sprite's colours related. Keep one light direction
  for the whole game.
* Animation: `frame`, `frame from N` (copy a frame and change it) and `finish` (shade
  and outline every frame the same way).
* Backgrounds: `background NAME WxH`; `tiles W H` for tilesets; `wrap` for textures
  that tile (drawing and `noise` wrap around the edges, so there is no seam).
* Patterns and procedural textures: `stripes`, `ridge` (hills), `scatter` (ore, trees,
  stars), `bevel`, and `field`, which paints a box from a formula such as
  `"sin(x*tau/128*2)"` mapped onto colour stops; `f` is the frame number, so a
  `field` can animate. Use `lock` (paint only inside what is drawn), `clip` and
  `mode multiply|add|screen` to shade and texture inside shapes.
* Deepcore's `art-src/` is a complete example: every sprite, the tileset, animated
  seamless water and lava, and a 960x540 title painting. It is in the Deepcore
  repository; clone it into `games/deepcore` to read it.
* `gmk-art build games/mygame` writes every resource into the folder: frames,
  origin, mask and bbox. New sprites and backgrounds are added, existing ones replaced
  by name. Re-run it after every edit. `gmk-art check` reports `.art` files that were
  changed but not built.
* **Order**: you may write objects and rooms that use `spr_ship` before it exists.
  `gmk-check` and the runner then fail with "unknown sprite" until you run
  `gmk-art build`, which creates it. So after adding art, build first, then check.
* `build` reads and writes the whole folder the way the IDE saves it: besides the art,
  it may fill in metadata the folder lacks (a `.toml` for a script that has none, slot
  counts in `project.toml`). That changes nothing in the game.
* Two traps, both in the format reference:
  - **`lock` first.** `lock` keeps painting inside what is already drawn. It must come
    *before* the texture (`circle ...`, `lock`, `noise ...`), and it only confines to
    a shape on an empty canvas. To shade one shape over a filled background, draw it
    as a `piece` and `stamp` it.
  - **Clear stops replace.** A colour with alpha (`#fff@0`, `clear`) *replaces* pixels
    in the default `mode normal`, punching holes. To lay translucent colour over what
    is there (clouds, stars, haze), set `mode over` first.

### 8.2 Looking: `gmk-art sheet`

```sh
gmk-art sheet games/mygame -o "$T/sheet.png"                    # everything
gmk-art sheet games/mygame --only spr_hero,spr_bat -o "$T/s.png" # bigger, with motion
gmk-art sheet games/mygame/art-src/hero.art -o "$T/hero.png"    # one file, no build
```

Then **view the PNG**. Each card shows the sprite zoomed in against the room it is
used in (background colour, layers and tiles as the player sees them), on light and on
dark, as a silhouette with its collision box and origin, and at 1x and 2x. Judge it at
1x: that is what the player sees. A sprite that only reads when zoomed is too
detailed. With `--only` (up to three names) or `--detail`, the card also compares each
frame with the previous one:
- red: pixels that disappeared;
- green: pixels that appeared;
- yellow: pixels that changed colour.

Use it to check an animation without watching it, including the loop from the last
frame to the first.

Sheets can run to several pages (`sheet.png`, `sheet-2.png`, ...). Write them to a
scratch directory, not into the game folder.

### 8.3 Measuring: `gmk-art check`

```sh
gmk-art check games/mygame               # warnings and notes; exit code 1 on warnings
gmk-art check games/mygame --verbose     # plus a table: colours, outline, edge contrast, scene
```

Act on the warnings:

* `contrast`: the sprite's edge blends into the backdrop. Add an outline, or a lit or
  shaded rim (`shade`). The template's first player sprite had a navy outline on a
  near-black room; this check caught it.
* `flip-origin` / `rotate-origin`: an object flips or rotates the sprite around an
  origin that is off-centre, so the sprite jumps.
* `seam`: a tiled background shows its edges.
* `not-built`: run `gmk-art build`.

The notes cover style:
- outlines 2 px thick (usually from outlining twice);
- sprites without the outline the rest of the game uses;
- sprites drawn at a different pixel size;
- stray pixels, anti-aliased fringes, and near-identical colours.

The backdrop is guessed from the rooms where the sprite's objects are placed. When the
guess is wrong (a cave creature judged against the sky), give it `against bg_rock` in
its `.art` file, or pass `--bg`.

### 8.4 Targets

* Readable at 1x against its real backdrop: median edge contrast 2:1 or more.
* One light direction, one outline style and one pixel size across the game.
* Few colours per sprite, taken from ramps. Similar things share ramps; important things
  (the player, pickups, danger) get colours nobody else uses.
* Origins at the centre (top-down) or bottom centre (platformers), so flipping and
  rotating stay in place.
* Animation: every frame changes something (look at the motion panels), and the last
  frame leads back into the first.

---

## 9. Sound and music

The gamekit's `crates/instance-music` provides two tools for making and checking sound
without listening to it. The notation reference is
[`crates/instance-music/README.md`](../crates/instance-music/README.md), with example
songs in `crates/instance-music/examples/` and `music_demos/`.

### 9.1 `gmk-music`: music as text

```sh
gmk-music render games/mygame/music-src/mus_title.txt -o games/mygame/sounds/mus_title.wav --loop --rate 32000 --mono
gmk-music render SONG.txt -o song.mid            # tiny MIDI file (kind 1 sound)
gmk-music info SONG.txt                          # length, ranges, density, mistakes
gmk-music instruments | drums | synths           # lists
```

* The notation has tracks with General MIDI instruments, notes like `E5q. D5e`, chord
  symbols `{Am7}w`, accompaniment patterns played `over | Am | F | C | G |`, drum grids,
  sections, `arrange |: A B :|`, and `variant`s that derive per-area versions from one
  theme.
* `--loop` renders a seamless loop of the `|: :|` region. Check that `render --loop`
  reports `loop seam: smooth`.
* Render flags: `--variant NAME|all`, `--soundfont PATH`, `--rate 44100|22050` (other
  rates such as 32000 work too), `--mono`, `--normalize DBFS|off` (the default is a
  peak of -1 dBFS) and `--tail SECS`.
* General MIDI patches sound like "MIDI". For synthwave or electronic music, give tracks
  `synth=PRESET` (supersaw, lead, pluck, pad, bass, drums, …; `gmk-music synths`). The
  built-in synthesiser has filter sweeps, sidechain `duck=`, chorus, delay and reverb,
  renders to WAV and needs no SoundFont.
* `gmk-music info` flags failed `|` bar checks, flat dynamics, silent bars and notes out
  of range. Run it after every edit.
* Workflow: keep the sources in `games/<name>/music-src/`, add a `README.txt` there with
  the exact render commands (as Deepcore's `music-src/README.txt` does), and render
  straight into `sounds/`. The first time, create the sound's TOML too (§3.3).

### 9.2 Sound effects as text

Sound effects are tiny songs: one or two `synth=` tracks (no SoundFont needed), a few
notes, a pitch glide or a drum hit, rendered without a loop and with a short tail for
the reverb to ring out. Keep them in `music-src/` next to the music:

```text
# music-src/snd_star.txt: a bright two-note chime for picking up a star
title "star pickup"
tempo 150
time 1/4                      # a one-beat bar, so a short sound is whole bars
track chime synth=fmpluck oct=6 vol=110 reverb=30
track glow  synth=bell oct=6 vol=80 reverb=40
chime: f E5s B5s> E6e^
glow:  mp rs rs [E6 B6]e
```

```sh
gmk-music render games/mygame/music-src/snd_star.txt -o games/mygame/sounds/snd_star.wav --rate 32000 --mono --tail 0.5
gmk-sounds games/mygame/sounds/snd_*.wav     # levels, and which effects sound alike
```

* Give each action its own character, such as a rising chime for a reward, a low
  falling glide (`glide=`) with drums for a crash, and a short click for UI. Then check
  that the `gmk-sounds` similarity table finds none alike.
* `time 1/4` (or `time 2/4`) keeps short sounds a whole number of bars; otherwise
  `gmk-music info` warns that the part is not a whole number of bars.
* Render at 22050 or 32000 Hz mono: effects are short, so the files stay small.
* Write the render commands into `music-src/README.txt`, as for music.

### 9.3 `gmk-sounds`: measuring sounds

```sh
gmk-pack games/mygame "$T/mygame.gm81" && gmk-sounds "$T/mygame.gm81"   # whole game
gmk-sounds games/mygame/sounds/snd_*.wav                                   # some files
gmk-sounds games/mygame/sounds/mus_title.wav --loop --per-second           # one music loop
```

`gmk-sounds` takes `.gm81`/`.gmk` files or plain sound files, not folders. Pack the game
into a scratch file to analyse all of its sounds together. Then it knows which sounds
loop: kind 1, `sound_loop` in the code, or "loop" in the name.

For each sound it reports peak and RMS in dBFS, clipping, silence, attack, spectral
centroid and band energies, and a one-line description ("short percussive bright
noise"). Loops get a seam verdict (`smooth`, `CLICK likely`, `GAP`, `FADES OUT`). A
similarity table flags sound effects that sound alike. Other flags are `--threshold`
(default 0.85), `--json`, `--no-similarity` and `--window SECS`. Act on its `check`
lines, its `loop` lines and its **THESE SOUND ALIKE** lines.

### 9.4 Targets

* Music: 60–120 s loops with bass, harmony, a melody built on a motif, drums, and
  dynamics that change from section to section. Match the genre to the theme. Give each
  area its own track (or variant) and crossfade over about 1 s.
* No clipping. Set levels by measuring, not by rendering everything as loud as
  possible: `gmk-music render` peak-normalises to -1 dBFS by default, which suits music
  but makes effects blast.
  - Music: about -18 to -20 dBFS RMS as it plays in the game (rendered at the default
    peak, then `sound_volume` 0.7–0.9).
  - Effects: render with `--normalize -4` to `-8`. A frequent effect (pickup, shot)
    should sit level with the music (its RMS within ~2 dB of the music's); a rare,
    big one (explosion, game over) at most ~4–6 dB above it.
  - Keep frequent effects out of the 3–6 kHz range where the ear is most sensitive:
    `gmk-sounds` reports the pitch and centroid (a "hissy tone ~5000 Hz" pickup
    sounds much louder than its level suggests).
  - Check the real mix: record a play session with `--audio-out mix.wav`, then
    `gmk-sounds mix.wav --per-second --window 0.25` shows each moment's level.
* Effects for different actions must not sound alike. Players notice: Deepcore's drill
  and thruster loops scored 0.91 and a player reported them as too similar.
* Headless runs can record the whole mix with `--audio-out` (§11.6).

---

## 10. Game design

### 10.1 Structure

* One **controller object** (`obj_game`) owns the globals, score, level flow, HUD and
  music. Give it a low object index, initialise **every** global (and every element of
  global arrays) in its Create, and either make it persistent and place it in a room
  that is entered once, or place it in every room and keep its state in globals.
* Use **parent objects** for shared behaviour. `obj_enemy` holds the Step and collision
  logic, and children override what differs and call `event_inherited()` to extend it.
  A collision event against a parent catches all its children.
* Put reusable code in **scripts** (`scr_player_move()`) and pass exactly the arguments
  they use.
* User events (`event_user(0)`) are cheap named callbacks inside an object.
* Prefix every name and avoid built-in names (§6.5).

### 10.2 Rooms and views

* Use one room per level (or one big generated room) plus a title room. Room creation
  code is a good place for per-level parameters (`global.level = 3;`). It runs *after*
  every Create event, so read those globals in Room Start or later.
* **Instance creation code** configures one placed instance (`amp = 70; spd = 1.6;`
  for a moving platform). It runs *before* that instance's Create event, so Create must
  read those variables and never assign them. Give every placed instance of such an
  object its creation code, or default the values in Create with
  `if (!variable_local_exists('amp')) amp = 40;`.
* Room speed 60 gives smooth action games; 30 is the default. Movement speeds are per
  step.
* For scrolling games, enable view 0, set its size and port, and either follow the
  player or move the view in code. Draw the HUD relative to the view (§7.6).

### 10.3 Sprites and collisions

* Put the **origin** where the object "stands": the centre for top-down games, the
  bottom centre for platformers. It is the pivot for rotation and scaling, and the x/y
  point. `gmk-art check` warns when a flipped or rotated sprite's origin is off-centre.
* Prefer **rectangle masks** for walls and players. They are fast and predictable. Use
  a dedicated mask sprite (`mask_sprite` in `object.toml`) so animation frames don't
  change the hitbox.
* Platformer movement: test `place_free(x + hspeed, y)` or `place_meeting` before
  moving, or use solid walls with `move_contact_solid(direction, speed)` in the
  collision event and then set the speed to zero.

### 10.4 Timing

Use alarms for delays and repeats (`alarm[0] = room_speed * 2`), time lines for
scripted sequences, and `current_time` (in ms) only for real-time measurements.

### 10.5 Performance

GML is interpreted. Keep per-step work modest:

* Keep the number of active instances in the hundreds. Deactivate far-away ones in big
  levels (`instance_deactivate_region` + `instance_activate_region`).
* Use tiles, backgrounds or a grid drawn in one Draw event for static scenery instead of
  one instance per block.
* Don't build strings every step. Cache the HUD text and update it when the value
  changes.
* Prefer `with (obj)` over looping with `instance_find`. Avoid `execute_string`.
* `INSTANCE_PROFILE=1 instance-runner …` prints per-event and per-function timings of
  your game.

### 10.6 Saving and files

* A folder game's working directory is **the game folder itself**, so the game writes
  its save files (`*.sav`, `*.ini`, `*.txt`, the error log) into `games/<name>/` when
  played. These are gitignored. Never commit them, and never let tests write there
  (§11.4).
* Save small state yourself with `ini_open`/`ini_write_real`/`ini_close` or
  `file_text_*`. `game_save`/`game_load` save the whole game state (not data structures
  or resources changed at run time). Turn off `let_f5_save_f6_load` in real games.
* Check `file_exists` before reading. `highscore_add` and `highscore_show` are built in.

### 10.7 Robustness

* Keep `display_errors` on. Turning on `treat_uninitialized_as_zero` hides bugs, so
  initialise variables instead.
* Guard references: `if (instance_exists(obj_player)) { … }` before `obj_player.x`.
  Remember there is no short-circuit evaluation.

### 10.8 Design lessons

These come from many play-test rounds with the user on Deepcore. Check new features
against them before showing them.

* **Choices come from limits, not prices.** Slots and capacity force decisions. Late in
  the game wealth makes prices meaningless. Consumables must not scale with upgrades,
  or they replace the upgrades. Give them flat amounts.
* **Every upgrade tier needs a visible, nameable new effect.** A bigger number alone
  ("range 8 → 11") reads as "what does this even do?".
* **Failure should be dramatic, never passive waiting.** Before failing, auto-spend the
  player's safety net (for example a fuel can).
* **Generous collision**: honour the player's intent, for example with corner roll-off
  when flying up past an edge or coyote time when jumping.
* **Protect safe zones** (spawn, shops) from the player's own destruction (digging,
  explosions).
* **Sound**: give actions punchy, distinct sounds. Ambient repeating effects (radar
  pings) get annoying. Throttle sounds that stack (many pickups in one step).
* **Cut UI friction and clutter**: re-open prompts, nag hints, oversized panels.
* **Simulations must settle to rest.** No flicker or stair-stepping. Hide the coarseness
  of a simulation with art.
* **Hazards should demand a tool or a decision**, not just deal damage.
* **Music**: adaptive per area, about 1 s crossfades, genre matched to the theme.

Working with play-test notes: implement concrete notes directly. For open design
problems, propose a recommended direction first. Verify each change visually with
headless screenshots of a crafted scene (§11).

---

## 11. Testing

### 11.1 Static check: `gmk-check`

Run `gmk-check games/<name>` after every edit. It takes a folder or a `.gm81`/`.gmk`
file. It compiles every code piece (scripts, events, room and instance creation code,
time lines, triggers, constants) and reports GameMaker's error messages with their
location. It also reports script argument count mismatches, broken references,
duplicate instance ids and name clashes. Warnings cover variables that are never
assigned, short-circuit hazards and similar problems.

```
error: obj_player / Step / action 1, line 12 col 2: Assignment operator expected. [compile]
    i++;
     ^
1 error, 0 warnings in 113 code pieces
```

Flags: `--pedantic` (also reports instance variables read in an event but not set in
the Create chain), `--strict` (warnings fail too), `--json`, `--quiet` (summary only).
Exit codes: 0 no errors, 1 errors (or warnings with `--strict`), 2 cannot load. It does
not find runtime errors such as wrong types or missing instances, so run the game too.

### 11.2 Headless runs

**Always** pass `--save-dir` with a fresh scratch directory and `--no-read-fallback` in
tests (§11.4 explains why). In the examples, `T` is a scratch directory outside the
game folder, for example `T=$(mktemp -d)`.

```sh
# smoke test: 300 steps, last frame saved
instance-runner games/mygame --headless --frames 300 --no-read-fallback --save-dir "$T/save" \
    --screenshot "$T/end.png" --log "$T/run.log" --max-seconds 60

# a typical test: through the title menu, cheat money in, watch it, screenshots at chosen steps
instance-runner games/mygame --headless --frames 600 --seed 1 \
    --no-read-fallback --save-dir "$T/save" \
    --input 30:press:enter --exec "120:global.cash = 100000" --watch 60:global.cash \
    --screenshot-at 200,600 "$T/shots" --no-draw --log "$T/run.log"

# monkey test: random input after the scripted start
instance-runner games/mygame --headless --frames 3000 --seed 1 \
    --no-read-fallback --save-dir "$T/save" \
    --input 30:press:enter --random-input 7@40 --max-seconds 120 --log "$T/run.log"

instance-runner games/mygame --debug     # windowed, with the Debug Information window
```

Output on stderr (and in the `--log` file): a summary line `ran N steps …; room R
(name), K instances, caption "…"`, every runtime error as `ERROR: …` / `FATAL ERROR: …`,
`show_debug_message` text, `show_message` text as `[message] …`, and a list of stubbed or
unsupported functions the game called. Questions are answered "yes" and `get_string`
returns its default. Exit codes: 0 ok, 1 load or start failure, 2 aborted by a fatal
error, 4 `--max-seconds` timeout. `GM_DUMP_INSTANCES=1` prints every instance (id,
object, position, sprite, depth, …) after the run.

On Windows the runner is a GUI program, and a shell may not capture its output. Use
`--log FILE` (or `2>&1 | cat`, or `Start-Process -Wait`).

In Git Bash on Windows, a path that is part of a larger argument, such as
`--exec-file 100:/c/tmp/x.gml`, is not converted to a Windows path, and the runner
reports "path not found". (Paths given as separate arguments are converted.) Write it
as `100:C:/tmp/x.gml`, or as `100:$(cygpath -m /c/tmp/x.gml)`.

### 11.3 Input

* `--input` takes comma-separated items keyed by step number. It can be repeated, and the
  scripts are combined:
  * `STEP:down:KEY` and `STEP:up:KEY`
  * `STEP:press:KEY` (down, then up two steps later)
  * `STEP:move:X:Y` and `STEP:click:X:Y` (left button down, up one step later)
  * `STEP:char:C`
  * joystick items: `STEP:joy1:button:N:down|up`, `STEP:joy1:axis:x:VALUE`,
    `STEP:joy1:pov:ANGLE`

  KEY is one of `left right up down space enter escape shift ctrl alt backspace tab`,
  `f1`…`f12`, a single letter or digit (`3` is the digit key), or a virtual-key code of
  two or more digits (`37`).
* `--random-input SEED[@START]` adds a deterministic "monkey" script from step START
  (default 3). It holds arrows, WASD, space, enter, Z, X and shift, and moves and clicks
  the mouse. Give a START after your scripted `--input` prefix (for example getting
  through the title menu) so the random keys don't interfere with it.
  `--random-keys LIST` replaces the key set (`--random-keys left,right,up,space,1,2`).
  **Leave `escape` out** if the title screen or pause menu quits on Escape, or the
  monkey will end the run early. `--random-no-mouse` drops mouse events. This is good
  for checking that nothing crashes.

### 11.4 Saves and files: `--save-dir` and `--no-read-fallback`

A folder game's `working_directory` is **the game folder itself** (`games/<name>/`).
Every file the game writes with a relative name (`ini_open('settings.ini')`,
`game_save('x.sav')`, text files, the error log, F9 screenshots) lands there. Those
files are the player's real saves and are gitignored.

* `--save-dir DIR` makes DIR the working directory (it is created if missing), so the
  test writes nowhere near the game folder. `program_directory` still names the game
  folder.
* Without `--no-read-fallback`, a relative file that is missing from DIR is **copied
  from the game folder on first use**. That includes the player's existing save, so the
  test silently starts from someone's progress. Always add `--no-read-fallback`, so only
  what is in DIR exists.
* Use a fresh DIR per test for reproducible runs.
* To test late content quickly, write a save or progress file into DIR before the run
  (e.g. "all levels unlocked"). Alternatively, jump directly with `--exec` (§11.5).

### 11.5 Injecting code and watching values

* `--exec STEP:CODE` (repeatable) and `--exec-file STEP:FILE.gml` run GML at the start
  of step STEP, before that step's events and input. STEP 0 means after the first room's
  Create, Game Start and Room Start events, before the first step. `self` is the first
  live instance of the current room, so don't rely on it. Name the target:
  `--exec "120:with (obj_player) { x = 100; }"`, `--exec "0:global.cash = 100000"`,
  `--exec "300:room_goto(rm_level3)"`. Errors print as `ERROR in --exec at step N:` and
  never stop the run.
* **Scene setup with `--exec-file`**: put the setup of a crafted scene in a scratch
  `.gml` file (place the player, spawn enemies, set upgrades, open a menu) and run it at
  a step after the game has started, for example
  `--exec-file 5:"$T/scene.gml" --screenshot-at 10,60 "$T/shots"`. This is the way to
  check a visual feature without playing up to it. Keep such files out of the game
  folder, and never add debug code to the game for this.
* `--watch K:EXPR` (repeatable) prints `watch step N: EXPR = value` every K steps
  (`--watch 60:global.cash`, `--watch 30:obj_player.x`,
  `--watch "60:instance_number(obj_enemy)"`). A watch that fails, for example because
  the instance doesn't exist yet, prints its error once. Use it to verify numbers:
  speeds, timers, economy, counts.
* `--exec-room ROOM` enters the first room as usual (Create, Game Start, Room Start, no
  step), then goes to ROOM, so the globals the first room sets up exist.
  `--exec-room-direct ROOM` starts in ROOM instead, and whatever the first room
  initialises is missing. Neither sets up state that a menu creates on Enter ("new
  game"). Script the menu with `--input`, or call the set-up script with `--exec 0:...`.
* `--seed N` seeds `randomize()` (including the one at game start) from N, so
  `random`, `irandom` and `choose` repeat exactly from run to run. Real-time sources
  (`current_time`, dates, `fps`) still vary.
* Self-test pattern: let the game print its own assertions with
  `show_debug_message('ok: …')` and grep the log.

### 11.6 Screenshots, speed, audio

* **Look at the screenshots.** They are PNGs you can view. Verify every visual change
  this way (layout, HUD, effects, colours at different depths and areas), together with
  the log's `ERROR` lines.
* `--screenshot OUT.png` saves the last frame. `--screenshot-at LIST DIR` saves the
  frames after the listed steps (`DIR/frame00060.png`, …). `--screenshot-every K DIR`
  saves every K-th frame. Pasting the frames into one contact sheet makes a long run
  quick to review.
* `--no-draw` renders only the frames that are saved, which is many times faster. Draw
  events still run, so game logic in Draw behaves the same, and with `--seed` the game
  state is identical. Code that reads the screen back (`draw_getpixel`, `screen_save`)
  sees the last rendered frame.
* Headless rendering is software rendering, and full-screen background layers are
  expensive there (roughly 15 ms per layer per frame). Expect slow rendered runs for
  games with several layers, and use `--no-draw` when you don't need every frame.
* `--audio-out FILE.wav` records the game's audio offline in game time: each step is
  exactly `1/room_speed` seconds, so the file is reproducible with `--seed`. MIDI needs
  `INSTANCE_SOUNDFONT`. At the end the runner prints the peak, RMS and clipped
  percentage. A peak above 0 dBFS or any clipping means the mix is too loud. Run
  `gmk-sounds FILE.wav --per-second` on it to see the loudness over time.
* `--max-seconds S` kills hung games (exit 4). Always set it in automated runs.

---

## 12. Where to look next

| Topic | Location |
|---|---|
| Project folder format in full | `docs/project-folder.md` in the Instance repository |
| GML language, runtime, built-in functions (`builtins.json`), sound, file formats | `specs/` in the Instance repository (`gml-language.md`, `runtime.md`, `builtins.json`, `sound.md`, …) |
| `.art` format, `gmk-art build`, `sheet`, `check` | [`crates/instance-art/README.md`](../crates/instance-art/README.md) |
| Music notation, `gmk-music`, `gmk-sounds` | [`crates/instance-music/README.md`](../crates/instance-music/README.md) |
| Example games | separate repositories cloned into `games/`: `deepcore`, `hoverdrome`, `lantern`, `neon_swarm`, `skyhook` |
| New game starter | [`games/_template/`](../games/_template/) |
| Art sources examples | the template's `art-src/sprites.art`; Deepcore's `art-src/` (a whole game, in the Deepcore repository) |
| Sound effects as text | §9.2; the template's `music-src/` |
| Music sources and render commands example | Deepcore's `music-src/` |
| Example songs | `crates/instance-music/examples/`, `music_demos/` |
