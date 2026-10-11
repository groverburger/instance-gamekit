# Template game

A minimal complete game to copy when starting a new one:

```sh
cp -r games/_template games/mygame     # then rename things (window caption, ini file name, ...)
git -C games/mygame init               # every game is its own repository
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

`project.toml` also holds the game settings (Esc does not end the game, vsync on) and
the room order. When you copy the template, give the new game its own `game_id` and
`guid` there. Adding resources (a script is just `scripts/<name>.gml`) is covered in
`docs/GUIDE.md`, section 3.
