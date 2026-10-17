# Template game

A minimal complete game to copy when starting a new one:

```sh
cp -r games/_template games/mygame     # then rename things (window caption, ini file name, ...)
git -C games/mygame init               # every game is its own repository
gmk-art build games/mygame             # after editing art-src/*.art
gmk-check games/mygame
instance-runner games/mygame
```

What it shows:

| Part | Where |
|---|---|
| Controller object: globals, pause (Esc), round timer, HUD; persistent, placed once in the first room | `objects/obj_game/` |
| Title screen: ENTER starts, Esc quits | `objects/obj_title/` |
| Player movement that respects the pause | `objects/obj_player/step.gml` |
| A collision event using `other` and `with` | `objects/obj_player/collision_obj_coin.gml` |
| An alarm that waits out the pause | `objects/obj_coin/` |
| Scripts (`argument0`, ...), a sound, two fonts, a constant (`ROUND_STEPS`, in `project.toml`) | `scripts/`, `sounds/`, `fonts/`, `project.toml` |
| A best score kept in an `.ini` file next to the game | `scripts/scr_save_best.gml` |
| The sprites, drawn as text: edit, then `gmk-art build games/<name>` | `art-src/sprites.art` |
| The coin sound as text (a sound effect): render commands in its `README.txt` | `music-src/` |
| The text shown by F1 (Game Information); replace it with your game's | `game_information.rtf` |

`project.toml` also holds the game settings (Esc does not end the game, so the close
button is not treated as Esc either and closes the window; vsync on) and
the room order. When you copy the template, give the new game its own `game_id` and
`guid` there. Adding resources (a script is just `scripts/<name>.gml`) is covered in
`docs/GUIDE.md`, section 3.
