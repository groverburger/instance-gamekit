# instance-art

Draw sprites and backgrounds as text, then **look** at them and **measure** them:

* `gmk-art build GAME` renders `GAME/art-src/**/*.art` and writes the sprites and
  backgrounds into the game folder. Existing resources are replaced by name, new ones
  are added. Code, rooms and objects are untouched.
* `gmk-art sheet GAME -o sheet.png` is a contact sheet of every sprite and background.
  It shows each sprite zoomed in:
  - against the room it appears in;
  - on light and on dark;
  - as a silhouette with its collision box and origin;
  - at 1× and 2×.

  Backgrounds that tile show their seams.
  `--only NAMES` (or `--detail`) adds frame-to-frame motion.
* `gmk-art check GAME` measures what is hard to judge by eye:
  - contrast against the backdrop;
  - stray pixels, soft edges and doubled outlines;
  - outline and pixel-size consistency;
  - origins that make flipping or rotating jump;
  - wrap seams;
  - strict palettes;
  - `.art` files that were changed but not built.

```sh
cargo build --release -p instance-art               # target/release/gmk-art(.exe)
gmk-art build games/mygame
gmk-art sheet games/mygame -o /tmp/sheet.png        # then look at the PNG
gmk-art check games/mygame                          # --verbose: a table of measurements
gmk-art sheet games/mygame/art-src/hero.art -o /tmp/hero.png   # a file, no game needed
```

`sheet` and `check` also accept `.gm81` files and loose `.art` files. Write sheets
outside the game folder (or name them `sheet*.png`, which the template's `.gitignore`
ignores).

---

## 1. A first sprite

```text
# art-src/hero.art
ramp abcde #3a78d8          # five blues, dark (a) to light (e), hue-shifted
color k #101828             # ink
color W #f4f4f8             # white

sprite spr_hero 16x16
origin center
rect 3 2 10 12 c            # body
grid 0 5                    # rows starting with | are pixels: the left eye
|.....W
|.....k
mirror x                    # copy the left half onto the right
shade tl                    # light from the top left
outline k                   # 1 px outline around everything
```

`gmk-art build games/mygame` turns this into `sprites/spr_hero/` (PNG and
`sprite.toml`, with origin and bounding box). Objects may refer to `spr_hero` before it
is built: until then `gmk-check` reports an unknown sprite, and `build` creates it. Each art pixel is one game pixel unless
`scale` says otherwise.

## 2. Files, lines, comments

* One command per line. Arguments are separated by spaces.
* A line starting with `#`, or a `#` token followed by a space (`rect 0 0 4 4 c  # body`),
  starts a comment. `#c82832` is a colour, not a comment.
* All `.art` files under `art-src/` are built, in any number of files. A name, pieces
  included, may be defined only once across all of them: prefix helper pieces
  (`title_hills`).
* `lock` keeps painting inside what is already drawn, so it confines texture to a shape
  only on a clear canvas. To shade one shape on top of a filled background, draw it as
  its own `piece` and `stamp` it.
* `use OTHER.art` (a path relative to this file) imports the other file's top-level
  colours and its resources, for `stamp`. Every file is read once, however many files
  `use` it.
* Errors name the file and line. Warnings, such as grid rows of different widths,
  don't stop the build.

## 3. Colours

A colour argument is one of:

| Form | Meaning |
|---|---|
| `k` | a one-character key defined with `color` or `ramp` |
| `#rgb`, `#rrggbb`, `#rrggbbaa` | a literal colour |
| `clear` | transparent (erases) |
| `k+1`, `k-2`, `#c82832-1` | that colour, a ramp step lighter or darker (hue-shifted) |
| `k*0.8`, `#c82832*1.1` | brightness times a factor (plain, not hue-shifted) |
| `k@50`, `#000@40` | that colour at 50% / 40% alpha |

Modifiers can be chained, left to right: `r-1*0.9@60`.

```text
color KEY COLOUR             define a key, e.g. color k #101828
ramp KEYS BASE [hue DEG]     define one key per character, dark to light
palette strict               (top level) this file's colours are the game palette
```

`ramp 12345 #c82832` gives five colours. The base sits at the middle key (`3`); for
an even count it's the key just past the middle. Darker steps turn towards violet and
lighter steps towards warm yellow, as painters shade, rather than mixing in black and
white. `hue` is the turn per step in degrees (default 8; 0 = plain lighter/darker).

Keys can be any character except `.`, `|`, `#` and space. Keys defined at the top of a
file are visible to all of its resources. Keys defined inside a resource are local to
it.

There are two traps:
- **Scope.** A `color` line written after a `sprite`/`piece` header belongs to that
  resource, even if it sits just above the next one. Put shared colours at the top of
  the file, or in a file you `use`.
- **Keys that look like flags.** A key whose name is also a word the command accepts
  (`h`, `v`, `d`, `a` for `stripes`; `dither`, `bands`, `smooth`) is read as that word.
  Pick other characters for such colours.

With `palette strict` in a file, `gmk-art check` reports every sprite and background
that uses a colour outside that file's top-level colours, and names the nearest
palette colour. Shading commands (`shade`, smooth gradients, `glow`, `+1` modifiers)
make new colours. Strict palettes go with grids, `bands` and `dither`.

## 4. Resources

```text
sprite NAME WxH              a sprite (written to sprites/NAME/)
background NAME WxH          a background (backgrounds/NAME.png)
piece NAME WxH               drawn only to be stamped into others; not written
```

Settings, anywhere inside the resource:

| Setting | Meaning |
|---|---|
| `origin X Y` / `origin center` / `origin bottom` | sprite origin in art pixels; `center` = (W/2, H/2), `bottom` = (W/2, H) (feet). Default 0 0 |
| `mask precise\|rect\|disk\|diamond` | collision shape (default `rect`) |
| `bbox auto\|full\|L T R B` | collision box: tight around the opaque pixels of all frames (default), the whole image, or manual (inclusive, art pixels) |
| `scale N` | every art pixel becomes N×N game pixels (chunky pixel art in a high-resolution game). Coordinates stay in art pixels |
| `tiles W H [HSEP VSEP HOFF VOFF]` | background: use as a tileset |
| `wrap` | drawing wraps around the edges, and `noise` becomes seamless: for tiling textures |
| `against SCENE ...` | the backdrop for `sheet` and `check`: a background or room name, or `#hex` |
| `icon` | sprite: `build` also makes frame 0 the game's icon (at most 256×256; 32×32 is usual) |

## 5. Drawing

Commands run in order on the canvas. Coordinates are art pixels, (0,0) top-left.
`X Y W H` boxes include W×H pixels. Shapes that take fractional numbers
(`circle`, `ellipse`, `ring`, `poly`, `glow`, `halo`) paint a pixel when its centre
is inside, so `circle 8 8 6` is centred in a 16×16 sprite.

| Command | Draws |
|---|---|
| `grid [X Y] [cell W [H]]` + rows `\|...\|` | pixels; `.` and space leave the pixel, other characters are colour keys. Rows may also start without `grid` (then at 0 0). A closing `\|` is optional; text after it is ignored. With `cell`, each character is a W×H cell: a colour key fills it, a `tile` key stamps a piece into it (logos from bricks, tile maps) |
| `tile KEY NAME [FRAME]` | makes KEY a grid key that stamps resource NAME (defined above) |
| `pixel X Y C` | one pixel |
| `rect X Y W H C` | a filled box |
| `box X Y W H C` | a 1 px rectangle outline |
| `clear X Y W H` | erases a box |
| `line X0 Y0 X1 Y1 C` | a 1 px line, both ends included |
| `circle CX CY R C [width W]` | a filled disc (`width`: only a ring W pixels wide inside its edge) |
| `ellipse CX CY RX RY C [angle DEG] [width W]` | a filled ellipse, rotated clockwise by DEG; `width` makes it a ring |
| `ring CX CY R0 R1 C` | a ring between two radii |
| `poly C X Y X Y X Y ...` | a filled polygon |
| `path C X Y X Y ...` | 1 px lines through the points (polyline) |
| `fill X Y C` | flood-fills the region of the colour at X,Y |
| `field X Y W H "EXPRESSION" C... [smooth\|bands\|dither] [cut T \| band LO HI]` | each pixel's value from a formula, picked among the stops (or, with `cut`/`band`, the last stop painted where the value is above T / between LO and HI). See below |
| `gradient X Y W H C C... [v\|h\|radial] [smooth\|bands\|dither]` | colour stops across a box: top to bottom (default), left to right, or centre outwards; mixed, in hard bands, or ordered dithering |
| `noise X Y W H C C... [scale S] [seed N] [octaves N] [smooth\|bands\|dither]` | fractal noise across the stops; `scale` is the feature size in pixels (default 8) |
| `noise X Y W H C cut T [scale S] [seed N]` | paints C only where the noise is above T (0..1): spots, ore veins, moss |
| `noise X Y W H C band LO HI [scale S] [seed N]` | paints C where the noise is between LO and HI: thin wandering lines (caustics, cracks, contours) |
| `ridge X Y W H C... [scale S] [seed N] [octaves N] [min F] [smooth\|bands\|dither]` | columns filled from a noisy top edge to the bottom of the box: hills, mountains, skylines. The edge sits between `min` (fraction of H, default 0) and the top; the stops run from the edge down. Seamless with `wrap` |
| `stripes X Y W H C C... [width N...] [h\|v\|d\|a] [offset N]` | repeating bands: horizontal, vertical, diagonal `/` (default) or `\`; `width` per colour (or one for all); measured from the canvas origin, so separate boxes line up: hazard tape, awnings, planks, grooves |
| `speckle X Y W H C DENSITY [seed N] [anywhere]` | random single pixels (DENSITY 0..1), only on opaque pixels unless `anywhere` |
| `bevel X Y W H [STEPS] [width N]` | lights the top and left N pixels of a box and shades the bottom and right ones: tiles, bricks, buttons |
| `shade DIR [STEPS]` | rim light: pixels facing the light (`tl t tr l r bl b br`) get a ramp step lighter, those facing away a step darker |
| `outline C [diagonal] [inside]` | paints the transparent pixels around the shape (`inside`: recolours the shape's own edge instead, for an inner rim) |
| `glow CX CY R C [K]` | adds light to opaque pixels, fading out over radius R |
| `halo CX CY R C [K]` | a soft translucent disc *behind* what is drawn (glows, soft shadows) |
| `shadow DX DY C` | the image's silhouette in C (use alpha, e.g. `#000@60`), offset, behind it: drop shadows |
| `recolor A B` | replaces one exact colour with another |
| `mirror x\|y` | copies the left half onto the right (`y`: the top onto the bottom) |
| `flip x\|y` | flips the canvas |
| `shift DX DY` | moves everything (wraps with `wrap`) |
| `rotate 90\|180\|270` | clockwise; 90/270 need a square canvas |
| `stamp NAME [FRAME] X Y` | draws another resource (finished frame, alpha-blended) at X Y |
| `scatter NAME X Y W H COUNT [seed N] [frame F \| vary] [surface] [spacing D]` | stamps NAME at COUNT random points in a box, its origin on the point; `vary` picks random frames; `surface` drops each onto the first opaque pixel below the top of the box (trees on hills); `spacing` keeps them D pixels apart: ore, stars, trees, rocks |

**Painting state.** Three commands change how the painting commands above (not
`shade`, `outline`, `glow` and the other effects, except that effects respect `clip`)
change pixels. The state lasts until changed, and resets at each `frame` and at `finish`:

| Command | Effect |
|---|---|
| `lock [on\|off]` | only pixels that are already opaque change, and they keep their alpha: shade or texture inside a shape with `gradient`, `noise`, `stripes` |
| `clip X Y W H` / `clip off` | only pixels inside the box change: half a circle, a texture confined to a panel |
| `mode normal\|over\|multiply\|add\|screen` | how a painted colour combines with the pixel: replace (default; stamps draw over), alpha-blend over, darken by multiplying, add light, lighten softly. The painted colour's alpha is the strength. `multiply`, `add` and `screen` leave clear pixels alone |

**Colour stops.** `gradient`, `noise`, `ridge` and `field` take two or more colours,
spread evenly over 0..1. A stop can name its position, `COLOUR:POSITION`: in
`gradient 0 0 64 64 #102040 #406080:0.8 #ffffff` the middle colour sits at 0.8. Stops
without a position are spread evenly between their neighbours. Two stops at nearly the
same position give a hard edge.

**`field` expressions** (quote them when they contain spaces). They use these values:
- `x`, `y`: the pixel centre;
- `u`, `v`: 0..1 across the box;
- `w`, `h`: the box size;
- `f`: the frame number (in `finish`, the frame being finished);
- `pi`, `tau`.

The operators are `+ - * / %` and `^`. The functions are `sin cos tan asin acos atan
atan2 abs sqrt exp ln pow min max clamp floor ceil round fract sign mix smoothstep step
noise(x, y[, seed]) hash(x, y[, seed])`. A pattern built from `sin(x*tau/W*k)` (whole
k) tiles across a W-wide canvas. Adding `f*pi/2` to the phase loops over four frames.
Deepcore's water and lava are drawn this way:

```text
piece water 128x128
frame
frame
frame
frame
finish
field 0 0 128 128 "(1-abs(sin(x*tau/64+f*pi/2)*sin(y*tau/64)))^6" #1c54a0 #46a0e1
```

Under `mode multiply` with stops `#000000 #ffffff`, a field becomes a brightness
factor (light and shadow from a formula). Under `mode over` with stops `C@0 C`, it
blends towards C by the value.

The order matters:
- `shade` and `outline` look at the shape as it is when they run, so draw the whole
  shape first.
- Use **one** `outline` at the end. Outlining twice makes a 2 px outline, and
  `check` reports it as `thick-outline`.
- `mirror` copies everything drawn so far.
- `lock` affects what comes *after* it. Draw the shape, then `lock`, then texture it:

  ```text
  circle 8 8 6 #8a6a50       # the rock
  lock                       # from here on, paint only on the rock
  noise 0 0 16 16 #6e5440 #a4846a scale 3
  lock off
  ```
- A colour with alpha (`#ffffff@40`, `clear`) **replaces** pixels in the default
  `mode normal`: noise or a gradient with translucent stops punches see-through holes
  into the sky behind it. To lay translucent colour over what is already drawn, switch
  to `mode over` first:

  ```text
  gradient 0 0 320 180 #05050f #141032       # the sky
  mode over
  noise 0 0 320 180 clear #4a3a8a@50 scale 40   # nebula haze over the sky
  mode normal
  ```

## 6. Frames

```text
sprite spr_bat 16x12
origin center
color p #5a3a7a
circle 8 6 3 p              # base: drawn into every frame
frame                       # frame 0: wings up
poly p 5 5 0 1 3 7
mirror x
frame                       # frame 1: wings down
poly p 5 6 0 10 3 6
mirror x
frame from 0                # frame 2: a copy of frame 0, then edits
shift 0 1
finish                      # runs on every frame after its own commands
shade tl
outline #120a18
```

* Commands before the first `frame` are the **base**; every new frame starts from it.
* `frame from N` starts from frame N (counting from 0) *as it was before `finish`*, so
  a frame can be "the previous one, with two pixels moved".
* `finish` commands run last on every frame, the place for `shade` and `outline`.
* With no `frame` at all, the base is the only frame.
* A background has one image; a sprite has any number of frames.

## 7. Contact sheets

`gmk-art sheet GAME|FILE.art ... [-o sheet.png] [--only NAMES] [--detail] [--zoom N]
[--bg SCENE] [--width PX]` writes `sheet.png`, plus `sheet-2.png` and so on when it
needs more pages. Each sprite card shows:

1. every frame zoomed against its **scene**. The scene is the room the sprite is placed
   in (or drawn from, via `draw_sprite`), rendered as the player sees it: background
   colour, background layers and tiles, inside view 0. Otherwise it's the game's
   largest room, or the sprite's `against` setting, or `--bg`;
2. frame 0 on light and on dark;
3. a silhouette with the collision box (red; the disk or diamond for those masks) and
   the origin (magenta cross);
4. all frames at 1× (and 2× for small sprites) on the scene: real size, the one that
   matters.

With `--detail` (automatic for `--only` with up to three names), the card adds a motion
panel per frame against the previous one:
- red: pixels that disappeared;
- green: pixels that appeared;
- yellow: pixels that changed colour;
- grey: pixels that stayed.

Read it to judge an animation without watching it. The last panel compares the last
frame with the first, which is the loop seam.

Background cards show the image (tileset grids in magenta). For a background tiled by
a room or marked `wrap`, the card also shows a 3×3 tiling (small images) or the corner
where four copies meet, enlarged.

The header line gives size, frame count, origin, mask, bbox, zoom and the scene used.

## 8. Checks

`gmk-art check GAME|FILE.art ... [--only NAMES] [--bg SCENE] [--verbose] [--json]`
exits with 1 when there are warnings.

| Code | Level | What |
|---|---|---|
| `contrast` | warning ≥ 75%, note ≥ 50% | share of the edge whose contrast with the scene's median brightness is below 1.5:1. The pixel one art pixel inwards also counts, so a dark outline with a lit rim passes. Aim for 2:1+. |
| `flip-origin` | warning | an object using the sprite sets `image_xscale`, but the origin isn't centred horizontally on the bbox, so the sprite jumps when it flips |
| `rotate-origin` | warning | an object sets `image_angle`, but the origin is far from the bbox centre |
| `seam` | warning | a background tiled by a room (or `wrap`) whose opposite edges differ much more than neighbouring pixels do |
| `off-palette` | warning | colours outside a `palette strict` palette |
| `not-built` | warning | an `.art` resource differs from the game's copy (run `build`) |
| `frame-size` | warning | frames of different sizes |
| `thick-outline` | note | the dark outline is 2 art pixels thick along much of the edge |
| `stray-pixel` | note | up to 6 isolated pixels |
| `soft-edge` | note | a few semi-transparent pixels on a hard-edged sprite (anti-aliasing fringe) |
| `near-duplicate` | note | in a sprite with at most 16 colours, colours too close to tell apart |
| `bbox-stale` | note | the stored automatic bbox doesn't match the image (the runner recomputes it) |
| `origin-outside` | note | origin outside the image |
| `no-outline` | note, game-wide | most sprites have a dark outline; these don't |
| `pixel-scale` | note, game-wide | most sprites are drawn with N×N pixels; these aren't |
| `tileset-size` | note | a tileset image isn't a whole number of tiles |

Measurements are made in art pixels: a sprite drawn with 2×2 blocks is measured at its
real resolution. `--verbose` prints the measurements per resource:
- size, frames and colours;
- pixel scale;
- outline colour;
- median edge contrast and the share of the edge that blends in;
- the scene;
- for backgrounds, how strong the seam is.

Contrast is judged against one scene per sprite. When the guess is wrong (a creature
that lives underground judged against the sky), use `against` in its `.art` file, or
`--bg` on the command line.
