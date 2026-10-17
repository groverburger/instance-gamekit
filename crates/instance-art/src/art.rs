//! The `.art` text format (see README.md): parsing and rendering.
//!
//! A file is read line by line. Colours (`color`, `ramp`) are resolved when they are
//! read; drawing commands are stored and run when the resource ends, frame by frame.

use crate::canvas::*;
use crate::color::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Sprite,
    Background,
    /// Drawn for `stamp` only; not written into the game.
    Piece,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    At(i32, i32),
    Center,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bbox {
    Auto,
    Full,
    Manual(i32, i32, i32, i32),
}

/// A rendered resource.
#[derive(Clone, Debug)]
pub struct Resource {
    pub kind: Kind,
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
    /// Size in art pixels (before `scale`).
    pub w: i32,
    pub h: i32,
    pub scale: i32,
    /// Origin in output pixels.
    pub origin: (i32, i32),
    /// GameMaker collision shape: 0 precise, 1 rectangle, 2 disk, 3 diamond.
    pub mask: i32,
    pub bbox: Bbox,
    /// Tileset settings: width, height, h separation, v separation, h offset, v offset.
    pub tiles: Option<[i32; 6]>,
    pub wrap: bool,
    /// `icon`: frame 0 is also the game's icon.
    pub icon: bool,
    /// Backgrounds (names or `#hex`) that `gmk-art check` and `sheet` show it against.
    pub against: Vec<String>,
    /// Frames as drawn, in art pixels (what `stamp` uses).
    pub art_frames: Vec<Canvas>,
    /// Frames as written into the game (scaled).
    pub frames: Vec<Canvas>,
}

impl Resource {
    /// The GameMaker bounding box (left, top, right, bottom) in output pixels.
    pub fn bbox_rect(&self) -> (i32, i32, i32, i32) {
        let (w, h) = (self.w * self.scale, self.h * self.scale);
        match self.bbox {
            Bbox::Full => (0, 0, w - 1, h - 1),
            Bbox::Manual(l, t, r, b) => (l * self.scale, t * self.scale, (r + 1) * self.scale - 1, (b + 1) * self.scale - 1),
            Bbox::Auto => {
                let mut acc: Option<(i32, i32, i32, i32)> = None;
                for f in &self.frames {
                    if let Some((l, t, r, b)) = f.opaque_box(0) {
                        acc = Some(match acc {
                            None => (l, t, r, b),
                            Some((l0, t0, r0, b0)) => (l0.min(l), t0.min(t), r0.max(r), b0.max(b)),
                        });
                    }
                }
                acc.unwrap_or((0, 0, 0, 0))
            }
        }
    }
}

/// Everything read from one `.art` file.
#[derive(Debug, Default)]
pub struct ArtFile {
    pub path: PathBuf,
    /// Top-level colours (visible to files that `use` this one).
    pub colors: HashMap<char, Rgba>,
    /// `palette strict`: the top-level colours are the game's palette.
    pub strict_palette: bool,
    pub resources: Vec<Rc<Resource>>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
enum Cmd {
    Grid { x: i32, y: i32, cell: (i32, i32), rows: Vec<Vec<Cell>> },
    Pixel(i32, i32, Rgba),
    Rect(i32, i32, i32, i32, Rgba),
    Box(i32, i32, i32, i32, Rgba),
    Line(i32, i32, i32, i32, Rgba),
    Ellipse { cx: f64, cy: f64, rx: f64, ry: f64, angle: f64, width: Option<f64>, c: Rgba },
    Ring(f64, f64, f64, f64, Rgba),
    Poly(Vec<(f64, f64)>, Rgba),
    Fill(i32, i32, Rgba),
    Gradient { x: i32, y: i32, w: i32, h: i32, stops: Vec<Rgba>, pos: Option<Vec<f64>>, dir: Direction, blend: Blend },
    Noise { x: i32, y: i32, w: i32, h: i32, stops: Vec<Rgba>, pos: Option<Vec<f64>>, scale: f64, seed: u32, octaves: u32, cut: Option<(f64, f64)>, blend: Blend },
    Ridge { x: i32, y: i32, w: i32, h: i32, stops: Vec<Rgba>, pos: Option<Vec<f64>>, scale: f64, seed: u32, octaves: u32, min: f64, blend: Blend },
    Field { x: i32, y: i32, w: i32, h: i32, expr: crate::expr::Expr, stops: Vec<Rgba>, pos: Option<Vec<f64>>, cut: Option<(f64, f64)>, blend: Blend },
    Stripes { x: i32, y: i32, w: i32, h: i32, stops: Vec<Rgba>, widths: Vec<i32>, dir: char, offset: i32 },
    Path(Vec<(i32, i32)>, Rgba),
    Bevel { x: i32, y: i32, w: i32, h: i32, steps: f64, width: i32 },
    Shadow(i32, i32, Rgba),
    Lock(bool),
    Clip(Option<(i32, i32, i32, i32)>),
    Mode(Mode),
    Scatter { name: String, x: i32, y: i32, w: i32, h: i32, count: u32, seed: u32, frame: Option<usize>, surface: bool, spacing: f64 },
    Speckle { x: i32, y: i32, w: i32, h: i32, c: Rgba, density: f64, seed: u32, anywhere: bool },
    Shade { dir: (i32, i32), steps: f64 },
    Outline { c: Rgba, diagonal: bool, inside: bool },
    Glow(f64, f64, f64, Rgba, f64),
    Halo(f64, f64, f64, Rgba, f64),
    Recolor(Rgba, Rgba),
    Mirror(bool),
    Flip(bool),
    Shift(i32, i32),
    Rotate(i32),
    Stamp { name: String, frame: usize, x: i32, y: i32 },
}

/// A grid cell: leave it, fill it with a colour, or stamp a piece into it.
#[derive(Clone, Debug)]
enum Cell {
    Leave,
    Color(Rgba),
    Stamp(String, usize),
}

struct FrameSpec {
    from: Option<usize>,
    cmds: Vec<(usize, Cmd)>,
}

/// A resource being read.
struct Pending {
    kind: Kind,
    name: String,
    line: usize,
    w: i32,
    h: i32,
    scale: i32,
    origin: Origin,
    mask: i32,
    bbox: Bbox,
    tiles: Option<[i32; 6]>,
    wrap: bool,
    icon: bool,
    against: Vec<String>,
    colors: HashMap<char, Rgba>,
    /// Grid keys that stamp a resource.
    tile_keys: HashMap<char, (String, usize)>,
    base: Vec<(usize, Cmd)>,
    frames: Vec<FrameSpec>,
    finish: Option<Vec<(usize, Cmd)>>,
}

impl Pending {
    fn cmds(&mut self) -> &mut Vec<(usize, Cmd)> {
        if let Some(f) = &mut self.finish {
            f
        } else if let Some(f) = self.frames.last_mut() {
            &mut f.cmds
        } else {
            &mut self.base
        }
    }
}

/// Reads `.art` files, following `use`; each file is read once.
#[derive(Default)]
pub struct Loader {
    files: HashMap<PathBuf, Rc<ArtFile>>,
    loading: Vec<PathBuf>,
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// A path for messages: relative to the current directory when possible.
pub fn shown(p: &Path) -> String {
    let s = p.display().to_string();
    let s = s.trim_start_matches(r"\\?\").to_string();
    if let Ok(cwd) = std::env::current_dir() {
        let cwd = canonical(&cwd).display().to_string();
        let cwd = cwd.trim_start_matches(r"\\?\");
        if let Some(rest) = s.strip_prefix(cwd) {
            return rest.trim_start_matches(['\\', '/']).replace('\\', "/");
        }
    }
    s.replace('\\', "/")
}

impl Loader {
    pub fn new() -> Self {
        Loader::default()
    }

    /// Every file loaded so far, in no particular order.
    pub fn files(&self) -> impl Iterator<Item = &Rc<ArtFile>> {
        self.files.values()
    }

    pub fn load(&mut self, path: &Path) -> Result<Rc<ArtFile>, String> {
        let key = canonical(path);
        if let Some(f) = self.files.get(&key) {
            return Ok(f.clone());
        }
        if self.loading.contains(&key) {
            return Err(format!("{}: `use` loop", shown(path)));
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", shown(path)))?;
        self.loading.push(key.clone());
        let result = self.parse(&text, &key);
        self.loading.pop();
        let file = Rc::new(result?);
        self.files.insert(key, file.clone());
        Ok(file)
    }

    /// Reads `.art` text as if it were the file `path` (for tests and tools).
    pub fn parse(&mut self, text: &str, path: &Path) -> Result<ArtFile, String> {
        let mut st = State {
            file: ArtFile { path: path.to_path_buf(), ..ArtFile::default() },
            known: HashMap::new(),
            cur: None,
            tiles: HashMap::new(),
            grid: None,
        };
        let lines: Vec<&str> = text.lines().collect();
        for (i, raw) in lines.iter().enumerate() {
            let ln = i + 1;
            let at = |msg: String| format!("{}:{ln}: {msg}", shown(path));
            let trimmed = raw.trim();
            if let Some(row) = trimmed.strip_prefix('|') {
                st.grid_row(row, ln).map_err(at)?;
                continue;
            }
            st.end_grid(ln).map_err(at)?;
            let toks = tokens(trimmed).map_err(at)?;
            if toks.is_empty() {
                continue;
            }
            if toks[0] == "use" {
                if toks.len() != 2 {
                    return Err(at("usage: use FILE".into()));
                }
                let target = path.parent().unwrap_or(Path::new(".")).join(toks[1]);
                let f = self.load(&target).map_err(|e| at(format!("in `use {}`: {e}", toks[1])))?;
                for (k, v) in &f.colors {
                    st.file.colors.insert(*k, *v);
                }
                for r in &f.resources {
                    st.known.insert(r.name.clone(), r.clone());
                }
                continue;
            }
            st.command(&toks, ln).map_err(at)?;
        }
        st.end_grid(lines.len() + 1).map_err(|e| format!("{}: {e}", shown(path)))?;
        st.finish_resource()?;
        Ok(st.file)
    }
}

/// Splits a line into tokens; `"..."` is one token (without the quotes), and a token
/// `#` starts a comment, as does a line starting with `#`.
fn tokens(line: &str) -> Result<Vec<&str>, String> {
    if line.starts_with('#') {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut rest = line;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        if let Some(q) = rest.strip_prefix('"') {
            let end = q.find('"').ok_or("a quoted token has no closing `\"`")?;
            out.push(&q[..end]);
            rest = &q[end + 1..];
            continue;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let t = &rest[..end];
        if t == "#" {
            break;
        }
        out.push(t);
        rest = &rest[end..];
    }
    Ok(out)
}

struct State {
    file: ArtFile,
    /// Resources `stamp` can use: this file's and those of used files.
    known: HashMap<String, Rc<Resource>>,
    cur: Option<Pending>,
    /// Top-level grid keys that stamp a resource.
    tiles: HashMap<char, (String, usize)>,
    /// A grid being read: (x, y, cell size, rows as text, first line).
    grid: Option<Grid>,
}

struct Grid {
    x: i32,
    y: i32,
    cell: (i32, i32),
    rows: Vec<String>,
    first: usize,
}

fn num<T: std::str::FromStr>(t: &str, what: &str) -> Result<T, String> {
    t.parse().map_err(|_| format!("{what}: `{t}` is not a number"))
}

fn direction(t: &str) -> Result<(i32, i32), String> {
    Ok(match t {
        "tl" | "top-left" => (-1, -1),
        "t" | "top" => (0, -1),
        "tr" | "top-right" => (1, -1),
        "l" | "left" => (-1, 0),
        "r" | "right" => (1, 0),
        "bl" | "bottom-left" => (-1, 1),
        "b" | "bottom" => (0, 1),
        "br" | "bottom-right" => (1, 1),
        _ => return Err(format!("`{t}` is not a direction (tl t tr l r bl b br)")),
    })
}

/// Splits `args` into positional values and `key value` options / flags.
struct Args<'a> {
    pos: Vec<&'a str>,
    opts: HashMap<&'a str, &'a str>,
    flags: Vec<&'a str>,
}

fn split_args<'a>(args: &[&'a str], keys: &[&str], flags: &[&str]) -> Result<Args<'a>, String> {
    let mut a = Args { pos: Vec::new(), opts: HashMap::new(), flags: Vec::new() };
    let mut i = 0;
    while i < args.len() {
        let t = args[i];
        if keys.contains(&t) {
            let v = args.get(i + 1).ok_or(format!("`{t}` needs a value"))?;
            a.opts.insert(t, v);
            i += 2;
        } else if flags.contains(&t) {
            a.flags.push(t);
            i += 1;
        } else {
            a.pos.push(t);
            i += 1;
        }
    }
    Ok(a)
}

fn blend_of(a: &Args) -> Blend {
    if a.flags.contains(&"dither") {
        Blend::Dither
    } else if a.flags.contains(&"bands") {
        Blend::Bands
    } else {
        Blend::Smooth
    }
}

impl State {
    fn color(&self, t: &str) -> Result<Rgba, String> {
        // a base (key, #hex, clear) then modifiers: +N / -N ramp steps lighter / darker,
        // *F brightness factor, @P alpha percent
        let split = t.char_indices().skip(1).find(|(_, c)| matches!(c, '+' | '-' | '*' | '@')).map(|(i, _)| i).unwrap_or(t.len());
        let (body, mut rest) = (&t[..split], &t[split..]);
        let mut c = if body == "clear" {
            CLEAR
        } else if body.starts_with('#') && body.len() > 1 {
            parse_hex(body).ok_or(format!("`{body}` is not a colour (#rgb, #rrggbb, #rrggbbaa)"))?
        } else {
            let mut ch = body.chars();
            let k = ch.next().ok_or("empty colour")?;
            if ch.next().is_some() {
                return Err(format!("`{t}` is not a colour: use a one-character key, #hex or `clear`"));
            }
            self.lookup(k).ok_or(format!("colour key `{k}` is not defined"))?
        };
        while let Some(op) = rest.chars().next() {
            let end = rest[1..].find(['+', '-', '*', '@']).map(|i| i + 1).unwrap_or(rest.len());
            let v: f64 = num(&rest[1..end], &format!("colour `{t}`"))?;
            match op {
                '+' => c = step(c, v, 8.0),
                '-' => c = step(c, -v, 8.0),
                '*' => {
                    for ch in c.iter_mut().take(3) {
                        *ch = (*ch as f64 * v).round().clamp(0.0, 255.0) as u8;
                    }
                }
                _ => c[3] = (c[3] as f64 * v / 100.0).round().clamp(0.0, 255.0) as u8,
            }
            rest = &rest[end..];
        }
        Ok(c)
    }

    /// Colour stops, each optionally `COLOUR:POSITION`. Positions that are left out are
    /// spread evenly between their neighbours (the first defaults to 0, the last to 1).
    fn stops(&self, toks: &[&str]) -> Result<(Vec<Rgba>, Option<Vec<f64>>), String> {
        let mut cols = Vec::new();
        let mut pos: Vec<Option<f64>> = Vec::new();
        for t in toks {
            match t.rsplit_once(':') {
                Some((c, p)) => {
                    cols.push(self.color(c)?);
                    pos.push(Some(num(p, &format!("stop `{t}`"))?));
                }
                None => {
                    cols.push(self.color(t)?);
                    pos.push(None);
                }
            }
        }
        if pos.iter().all(Option::is_none) {
            return Ok((cols, None));
        }
        let n = pos.len();
        if pos[0].is_none() {
            pos[0] = Some(0.0);
        }
        if pos[n - 1].is_none() {
            pos[n - 1] = Some(pos[0].unwrap().max(1.0));
        }
        let mut i = 0;
        while i < n {
            if pos[i].is_none() {
                let j = (i..n).find(|&j| pos[j].is_some()).unwrap();
                let (a, b) = (pos[i - 1].unwrap(), pos[j].unwrap());
                for (k, p) in pos.iter_mut().enumerate().take(j).skip(i) {
                    *p = Some(a + (b - a) * (k - i + 1) as f64 / (j - i + 1) as f64);
                }
                i = j;
            }
            i += 1;
        }
        let pos: Vec<f64> = pos.into_iter().map(Option::unwrap).collect();
        if pos.windows(2).any(|w| w[1] < w[0]) {
            return Err("stop positions must not decrease".into());
        }
        Ok((cols, Some(pos)))
    }

    fn lookup_tile(&self, k: char) -> Option<(String, usize)> {
        if let Some(p) = &self.cur {
            if let Some(t) = p.tile_keys.get(&k) {
                return Some(t.clone());
            }
            if p.colors.contains_key(&k) {
                return None;
            }
        }
        self.tiles.get(&k).cloned()
    }

    fn lookup(&self, k: char) -> Option<Rgba> {
        if let Some(p) = &self.cur {
            if let Some(c) = p.colors.get(&k) {
                return Some(*c);
            }
        }
        self.file.colors.get(&k).copied()
    }

    fn define(&mut self, k: char, c: Rgba) -> Result<(), String> {
        if k == '.' || k == '|' || k == '#' {
            return Err(format!("`{k}` cannot be a colour key"));
        }
        match &mut self.cur {
            Some(p) => {
                p.tile_keys.remove(&k);
                p.colors.insert(k, c);
            }
            None => {
                self.tiles.remove(&k);
                self.file.colors.insert(k, c);
            }
        };
        Ok(())
    }

    fn grid_row(&mut self, row: &str, ln: usize) -> Result<(), String> {
        let row = match row.find('|') {
            Some(i) => &row[..i],
            None => row.trim_end(),
        };
        match &mut self.grid {
            Some(g) => g.rows.push(row.to_string()),
            None => {
                if self.cur.is_none() {
                    return Err("grid row outside a sprite or background".into());
                }
                self.grid = Some(Grid { x: 0, y: 0, cell: (1, 1), rows: vec![row.to_string()], first: ln });
            }
        }
        Ok(())
    }

    fn end_grid(&mut self, ln: usize) -> Result<(), String> {
        let Some(Grid { x, y, cell, rows, first }) = self.grid.take() else { return Ok(()) };
        if rows.is_empty() {
            return Err(format!("`grid` on line {} has no rows (rows start with `|`)", ln - 1));
        }
        let mut out = Vec::new();
        for r in &rows {
            let mut row = Vec::new();
            for ch in r.chars() {
                row.push(match ch {
                    '.' | ' ' => Cell::Leave,
                    k => match self.lookup_tile(k) {
                        Some((name, frame)) => Cell::Stamp(name, frame),
                        None => Cell::Color(self.lookup(k).ok_or(format!("grid (line {first}): key `{k}` is not defined"))?),
                    },
                });
            }
            out.push(row);
        }
        let widths: Vec<usize> = out.iter().map(Vec::len).collect();
        let (lo, hi) = (*widths.iter().min().unwrap(), *widths.iter().max().unwrap());
        let p = self.cur.as_mut().unwrap();
        if lo != hi {
            let odd: Vec<String> =
                widths.iter().enumerate().filter(|(_, w)| **w != hi).map(|(i, w)| format!("line {} is {w}", first + i)).collect();
            self.file.warnings.push(format!(
                "{}:{first}: grid rows have different widths (most are {hi}; {})",
                shown(&self.file.path),
                odd.join(", ")
            ));
        }
        if x + hi as i32 * cell.0 > p.w || y + out.len() as i32 * cell.1 > p.h {
            self.file.warnings.push(format!(
                "{}:{first}: grid ({hi}x{} cells of {}x{}) at {x},{y} goes past the {}x{} canvas",
                shown(&self.file.path),
                out.len(),
                cell.0,
                cell.1,
                p.w,
                p.h
            ));
        }
        p.cmds().push((first, Cmd::Grid { x, y, cell, rows: out }));
        Ok(())
    }

    fn need_cur(&mut self, what: &str) -> Result<&mut Pending, String> {
        self.cur.as_mut().ok_or(format!("`{what}` outside a sprite or background"))
    }

    fn command(&mut self, t: &[&str], ln: usize) -> Result<(), String> {
        let cmd = t[0];
        let args = &t[1..];
        let n = |i: usize| -> Result<i32, String> { num(args.get(i).ok_or(format!("`{cmd}`: missing argument {}", i + 1))?, cmd) };
        let f = |i: usize| -> Result<f64, String> { num(args.get(i).ok_or(format!("`{cmd}`: missing argument {}", i + 1))?, cmd) };
        let c = |s: &State, i: usize| -> Result<Rgba, String> { s.color(args.get(i).ok_or(format!("`{cmd}`: missing colour"))?) };
        let exact = |k: usize, usage: &str| -> Result<(), String> {
            if args.len() == k {
                Ok(())
            } else {
                Err(format!("usage: {usage}"))
            }
        };
        let draw = match cmd {
            "sprite" | "background" | "piece" => {
                self.finish_resource()?;
                let usage = format!("{cmd} NAME WIDTHxHEIGHT");
                if args.len() != 2 {
                    return Err(format!("usage: {usage}"));
                }
                let (w, h) = args[1].split_once('x').ok_or(format!("usage: {usage}"))?;
                let (w, h): (i32, i32) = (num(w, "width")?, num(h, "height")?);
                if w <= 0 || h <= 0 || w > 4096 || h > 4096 {
                    return Err(format!("size {w}x{h} out of range"));
                }
                let name = args[0].to_string();
                if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') || name.starts_with(|c: char| c.is_ascii_digit()) {
                    return Err(format!("`{name}` is not a valid resource name (letters, digits, _)"));
                }
                if self.file.resources.iter().any(|r| r.name == name) {
                    return Err(format!("`{name}` is defined twice in this file"));
                }
                self.cur = Some(Pending {
                    kind: match cmd {
                        "sprite" => Kind::Sprite,
                        "background" => Kind::Background,
                        _ => Kind::Piece,
                    },
                    name,
                    line: ln,
                    w,
                    h,
                    scale: 1,
                    origin: Origin::At(0, 0),
                    mask: 1,
                    bbox: Bbox::Auto,
                    tiles: None,
                    wrap: false,
                    icon: false,
                    against: Vec::new(),
                    colors: HashMap::new(),
                    tile_keys: HashMap::new(),
                    base: Vec::new(),
                    frames: Vec::new(),
                    finish: None,
                });
                return Ok(());
            }
            "palette" => {
                if self.cur.is_some() || args != ["strict"] {
                    return Err("usage: palette strict (at the top level of a file)".into());
                }
                self.file.strict_palette = true;
                return Ok(());
            }
            "color" | "colour" => {
                exact(2, "color KEY COLOUR")?;
                let mut ch = args[0].chars();
                let k = ch.next().unwrap();
                if ch.next().is_some() {
                    return Err(format!("colour key `{}` must be one character", args[0]));
                }
                let v = c(self, 1)?;
                self.define(k, v)?;
                return Ok(());
            }
            "ramp" => {
                let a = split_args(args, &["hue"], &[])?;
                if a.pos.len() != 2 {
                    return Err("usage: ramp KEYS BASE [hue DEGREES]   e.g. ramp 12345 #c82832".into());
                }
                let keys: Vec<char> = a.pos[0].chars().collect();
                if keys.len() < 2 {
                    return Err("a ramp needs at least two keys".into());
                }
                let base = self.color(a.pos[1])?;
                let hue = a.opts.get("hue").map(|h| num(h, "hue")).transpose()?.unwrap_or(8.0);
                for (k, v) in keys.iter().zip(ramp(base, keys.len(), hue)) {
                    self.define(*k, v)?;
                }
                return Ok(());
            }
            "origin" => {
                let p = self.need_cur(cmd)?;
                p.origin = match args {
                    ["center"] | ["centre"] => Origin::Center,
                    ["bottom"] => Origin::Bottom,
                    [x, y] => Origin::At(num(x, "origin")?, num(y, "origin")?),
                    _ => return Err("usage: origin X Y | origin center | origin bottom".into()),
                };
                return Ok(());
            }
            "mask" => {
                let p = self.need_cur(cmd)?;
                p.mask = match args {
                    ["precise"] => 0,
                    ["rect"] | ["rectangle"] => 1,
                    ["disk"] => 2,
                    ["diamond"] => 3,
                    _ => return Err("usage: mask precise|rect|disk|diamond".into()),
                };
                return Ok(());
            }
            "bbox" => {
                let v = match args {
                    ["auto"] => Bbox::Auto,
                    ["full"] => Bbox::Full,
                    [l, t, r, b] => Bbox::Manual(num(l, "bbox")?, num(t, "bbox")?, num(r, "bbox")?, num(b, "bbox")?),
                    _ => return Err("usage: bbox auto | bbox full | bbox LEFT TOP RIGHT BOTTOM".into()),
                };
                self.need_cur(cmd)?.bbox = v;
                return Ok(());
            }
            "scale" => {
                exact(1, "scale N")?;
                let s = n(0)?;
                if !(1..=16).contains(&s) {
                    return Err("scale must be 1..16".into());
                }
                self.need_cur(cmd)?.scale = s;
                return Ok(());
            }
            "tiles" => {
                if args.len() != 2 && args.len() != 6 {
                    return Err("usage: tiles W H [HSEP VSEP HOFF VOFF]".into());
                }
                let mut v = [0; 6];
                for (i, slot) in v.iter_mut().enumerate().take(args.len()) {
                    *slot = n(i)?;
                }
                let p = self.need_cur(cmd)?;
                if p.kind != Kind::Background {
                    return Err("`tiles` is for backgrounds".into());
                }
                p.tiles = Some(v);
                return Ok(());
            }
            "wrap" => {
                exact(0, "wrap")?;
                self.need_cur(cmd)?.wrap = true;
                return Ok(());
            }
            "against" => {
                if args.is_empty() {
                    return Err("usage: against BACKGROUND|#hex ...".into());
                }
                for a in args {
                    if a.starts_with('#') {
                        parse_hex(a).ok_or(format!("`{a}` is not a colour"))?;
                    }
                }
                self.need_cur(cmd)?.against.extend(args.iter().map(|s| s.to_string()));
                return Ok(());
            }
            "icon" => {
                exact(0, "icon")?;
                let p = self.need_cur(cmd)?;
                if p.kind != Kind::Sprite {
                    return Err("`icon` is for sprites".into());
                }
                p.icon = true;
                return Ok(());
            }
            "tile" => {
                let (k, name, frame) = match args {
                    [k, name] => (k, name, 0),
                    [k, name, f] => (k, name, num(f, "frame")?),
                    _ => return Err("usage: tile KEY NAME [FRAME]   (a grid key that stamps NAME)".into()),
                };
                let mut ch = k.chars();
                let key = ch.next().unwrap();
                if ch.next().is_some() || matches!(key, '.' | '|' | '#') {
                    return Err(format!("`{k}` cannot be a grid key"));
                }
                let r = self.known.get(*name).ok_or(format!("`{name}` is not defined above (or in a `use`d file)"))?;
                if frame >= r.art_frames.len() {
                    return Err(format!("`{name}` has {} frame(s)", r.art_frames.len()));
                }
                let entry = (name.to_string(), frame);
                match &mut self.cur {
                    Some(p) => {
                        p.colors.remove(&key);
                        p.tile_keys.insert(key, entry);
                    }
                    None => {
                        self.file.colors.remove(&key);
                        self.tiles.insert(key, entry);
                    }
                }
                return Ok(());
            }
            "frame" => {
                let p = self.need_cur(cmd)?;
                if p.finish.is_some() {
                    return Err("`frame` after `finish`: put `finish` after the last frame".into());
                }
                let from = match args {
                    [] => None,
                    ["from", k] => {
                        let k: usize = num(k, "frame")?;
                        if k >= p.frames.len() {
                            return Err(format!("frame {k} does not exist yet (frames count from 0)"));
                        }
                        Some(k)
                    }
                    _ => return Err("usage: frame | frame from N".into()),
                };
                p.frames.push(FrameSpec { from, cmds: Vec::new() });
                return Ok(());
            }
            "finish" => {
                exact(0, "finish")?;
                let p = self.need_cur(cmd)?;
                if p.finish.is_some() {
                    return Err("`finish` twice".into());
                }
                p.finish = Some(Vec::new());
                return Ok(());
            }
            "grid" => {
                self.need_cur(cmd)?;
                let usage = "usage: grid [X Y] [cell W [H]], then rows starting with |";
                let (pos, cell) = match args.iter().position(|a| *a == "cell") {
                    Some(i) => {
                        let c = &args[i + 1..];
                        let cell = match c {
                            [w] => (num(w, "cell")?, num(w, "cell")?),
                            [w, h] => (num(w, "cell")?, num(h, "cell")?),
                            _ => return Err(usage.into()),
                        };
                        (&args[..i], cell)
                    }
                    None => (args, (1, 1)),
                };
                if cell.0 < 1 || cell.1 < 1 {
                    return Err("cell size must be at least 1".into());
                }
                let (x, y) = match pos {
                    [] => (0, 0),
                    [x, y] => (num(x, "grid")?, num(y, "grid")?),
                    _ => return Err(usage.into()),
                };
                self.grid = Some(Grid { x, y, cell, rows: Vec::new(), first: ln + 1 });
                return Ok(());
            }
            "pixel" => {
                exact(3, "pixel X Y COLOUR")?;
                Cmd::Pixel(n(0)?, n(1)?, c(self, 2)?)
            }
            "rect" => {
                exact(5, "rect X Y W H COLOUR")?;
                Cmd::Rect(n(0)?, n(1)?, n(2)?, n(3)?, c(self, 4)?)
            }
            "clear" => {
                exact(4, "clear X Y W H")?;
                Cmd::Rect(n(0)?, n(1)?, n(2)?, n(3)?, CLEAR)
            }
            "box" => {
                exact(5, "box X Y W H COLOUR")?;
                Cmd::Box(n(0)?, n(1)?, n(2)?, n(3)?, c(self, 4)?)
            }
            "line" => {
                exact(5, "line X0 Y0 X1 Y1 COLOUR")?;
                Cmd::Line(n(0)?, n(1)?, n(2)?, n(3)?, c(self, 4)?)
            }
            "circle" | "ellipse" => {
                let a = split_args(args, &["angle", "width"], &[])?;
                let n_pos = if cmd == "circle" { 4 } else { 5 };
                if a.pos.len() != n_pos {
                    return Err(if cmd == "circle" {
                        "usage: circle CX CY R COLOUR [width W]".into()
                    } else {
                        "usage: ellipse CX CY RX RY COLOUR [angle DEG] [width W]".into()
                    });
                }
                let g = |i: usize| num::<f64>(a.pos[i], cmd);
                let (rx, ry) = if cmd == "circle" { (g(2)?, g(2)?) } else { (g(2)?, g(3)?) };
                Cmd::Ellipse {
                    cx: g(0)?,
                    cy: g(1)?,
                    rx,
                    ry,
                    angle: a.opts.get("angle").map(|v| num(v, "angle")).transpose()?.unwrap_or(0.0),
                    width: a.opts.get("width").map(|v| num(v, "width")).transpose()?,
                    c: self.color(a.pos[n_pos - 1])?,
                }
            }
            "ring" => {
                exact(5, "ring CX CY R0 R1 COLOUR")?;
                Cmd::Ring(f(0)?, f(1)?, f(2)?, f(3)?, c(self, 4)?)
            }
            "poly" => {
                if args.len() < 7 || args.len().is_multiple_of(2) {
                    return Err("usage: poly COLOUR X Y X Y X Y ... (at least 3 points)".into());
                }
                let col = c(self, 0)?;
                let mut pts = Vec::new();
                for i in (1..args.len()).step_by(2) {
                    pts.push((f(i)?, f(i + 1)?));
                }
                Cmd::Poly(pts, col)
            }
            "fill" => {
                exact(3, "fill X Y COLOUR")?;
                Cmd::Fill(n(0)?, n(1)?, c(self, 2)?)
            }
            "gradient" => {
                let a = split_args(args, &[], &["v", "h", "radial", "dither", "bands", "smooth"])?;
                if a.pos.len() < 6 {
                    return Err("usage: gradient X Y W H COLOUR COLOUR... [v|h|radial] [smooth|bands|dither]".into());
                }
                let p = |i: usize| num::<i32>(a.pos[i], cmd);
                let (stops, pos) = self.stops(&a.pos[4..])?;
                let dir = if a.flags.contains(&"h") {
                    Direction::Horizontal
                } else if a.flags.contains(&"radial") {
                    Direction::Radial
                } else {
                    Direction::Vertical
                };
                Cmd::Gradient { x: p(0)?, y: p(1)?, w: p(2)?, h: p(3)?, stops, pos, dir, blend: blend_of(&a) }
            }
            "noise" => {
                // `band LO HI` takes two values, so take it out before splitting
                let mut args: Vec<&str> = args.to_vec();
                let mut cut = None;
                if let Some(i) = args.iter().position(|a| *a == "band") {
                    let lo: f64 = num(args.get(i + 1).ok_or("`band` needs LO HI")?, "band")?;
                    let hi: f64 = num(args.get(i + 2).ok_or("`band` needs LO HI")?, "band")?;
                    cut = Some((lo, hi));
                    args.drain(i..i + 3);
                }
                if let Some(i) = args.iter().position(|a| *a == "cut") {
                    let lo: f64 = num(args.get(i + 1).ok_or("`cut` needs a value")?, "cut")?;
                    cut = Some((lo, 1.0));
                    args.drain(i..i + 2);
                }
                let a = split_args(&args, &["scale", "seed", "octaves"], &["dither", "bands", "smooth"])?;
                if a.pos.len() < 5 {
                    return Err("usage: noise X Y W H COLOUR... [scale S] [seed N] [octaves N] [cut T | band LO HI] [smooth|bands|dither]".into());
                }
                let p = |i: usize| num::<i32>(a.pos[i], cmd);
                let (stops, pos) = self.stops(&a.pos[4..])?;
                if stops.len() == 1 && cut.is_none() {
                    return Err("noise with one colour needs `cut T` (paints above T) or `band LO HI` (paints between)".into());
                }
                let scale = a.opts.get("scale").map(|v| num::<f64>(v, "scale")).transpose()?.unwrap_or(8.0);
                if scale <= 0.0 {
                    return Err("scale must be positive".into());
                }
                Cmd::Noise {
                    x: p(0)?,
                    y: p(1)?,
                    w: p(2)?,
                    h: p(3)?,
                    stops,
                    pos,
                    scale,
                    seed: a.opts.get("seed").map(|v| num(v, "seed")).transpose()?.unwrap_or(1),
                    octaves: a.opts.get("octaves").map(|v| num(v, "octaves")).transpose()?.unwrap_or(3),
                    cut,
                    blend: blend_of(&a),
                }
            }
            "field" => {
                let mut args: Vec<&str> = args.to_vec();
                let mut cut = None;
                if let Some(i) = args.iter().position(|a| *a == "band") {
                    let lo: f64 = num(args.get(i + 1).ok_or("`band` needs LO HI")?, "band")?;
                    let hi: f64 = num(args.get(i + 2).ok_or("`band` needs LO HI")?, "band")?;
                    cut = Some((lo, hi));
                    args.drain(i..i + 3);
                }
                if let Some(i) = args.iter().position(|a| *a == "cut") {
                    let lo: f64 = num(args.get(i + 1).ok_or("`cut` needs a value")?, "cut")?;
                    cut = Some((lo, f64::INFINITY));
                    args.drain(i..i + 2);
                }
                let a = split_args(&args, &[], &["dither", "bands", "smooth"])?;
                if a.pos.len() < 6 {
                    return Err("usage: field X Y W H \"EXPRESSION\" COLOUR... [smooth|bands|dither] [cut T | band LO HI]".into());
                }
                let p = |i: usize| num::<i32>(a.pos[i], cmd);
                let expr = crate::expr::parse(a.pos[4]).map_err(|e| format!("field: {e}"))?;
                let (stops, pos) = self.stops(&a.pos[5..])?;
                Cmd::Field { x: p(0)?, y: p(1)?, w: p(2)?, h: p(3)?, expr, stops, pos, cut, blend: blend_of(&a) }
            }
            "ridge" => {
                let a = split_args(args, &["scale", "seed", "octaves", "min"], &["dither", "bands", "smooth"])?;
                if a.pos.len() < 5 {
                    return Err("usage: ridge X Y W H COLOUR... [scale S] [seed N] [octaves N] [min F] [smooth|bands|dither]".into());
                }
                let p = |i: usize| num::<i32>(a.pos[i], cmd);
                let (stops, pos) = self.stops(&a.pos[4..])?;
                Cmd::Ridge {
                    x: p(0)?,
                    y: p(1)?,
                    w: p(2)?,
                    h: p(3)?,
                    stops,
                    pos,
                    scale: a.opts.get("scale").map(|v| num::<f64>(v, "scale")).transpose()?.unwrap_or(32.0).max(0.001),
                    seed: a.opts.get("seed").map(|v| num(v, "seed")).transpose()?.unwrap_or(1),
                    octaves: a.opts.get("octaves").map(|v| num(v, "octaves")).transpose()?.unwrap_or(3),
                    min: a.opts.get("min").map(|v| num::<f64>(v, "min")).transpose()?.unwrap_or(0.0).clamp(0.0, 1.0),
                    blend: blend_of(&a),
                }
            }
            "stripes" => {
                // widths: `width N N ...` (one per colour, or one for all)
                let mut args: Vec<&str> = args.to_vec();
                let mut widths = Vec::new();
                if let Some(i) = args.iter().position(|a| *a == "width") {
                    let mut j = i + 1;
                    while j < args.len() && args[j].parse::<i32>().is_ok() {
                        widths.push(args[j].parse::<i32>().unwrap());
                        j += 1;
                    }
                    args.drain(i..j);
                }
                let a = split_args(&args, &["offset"], &["h", "v", "d", "a"])?;
                if a.pos.len() < 6 {
                    return Err("usage: stripes X Y W H COLOUR COLOUR... [width N...] [h|v|d|a] [offset N]".into());
                }
                let p = |i: usize| num::<i32>(a.pos[i], cmd);
                let stops = a.pos[4..].iter().map(|s| self.color(s)).collect::<Result<Vec<_>, _>>()?;
                if widths.is_empty() {
                    widths.push(1);
                }
                if widths.iter().any(|w| *w < 1) {
                    return Err("stripe widths must be at least 1".into());
                }
                let widths: Vec<i32> = (0..stops.len()).map(|i| if widths.len() == 1 { widths[0] } else { *widths.get(i).unwrap_or(&1) }).collect();
                let dir = ['h', 'v', 'd', 'a'].into_iter().find(|d| a.flags.contains(&d.to_string().as_str())).unwrap_or('d');
                Cmd::Stripes {
                    x: p(0)?,
                    y: p(1)?,
                    w: p(2)?,
                    h: p(3)?,
                    stops,
                    widths,
                    dir,
                    offset: a.opts.get("offset").map(|v| num(v, "offset")).transpose()?.unwrap_or(0),
                }
            }
            "path" => {
                if args.len() < 5 || args.len().is_multiple_of(2) {
                    return Err("usage: path COLOUR X Y X Y ... (at least 2 points)".into());
                }
                let col = c(self, 0)?;
                let mut pts = Vec::new();
                for i in (1..args.len()).step_by(2) {
                    pts.push((f(i)?.floor() as i32, f(i + 1)?.floor() as i32));
                }
                Cmd::Path(pts, col)
            }
            "bevel" => {
                let a = split_args(args, &["width"], &[])?;
                if a.pos.len() != 4 && a.pos.len() != 5 {
                    return Err("usage: bevel X Y W H [STEPS] [width N]".into());
                }
                let p = |i: usize| num::<i32>(a.pos[i], cmd);
                Cmd::Bevel {
                    x: p(0)?,
                    y: p(1)?,
                    w: p(2)?,
                    h: p(3)?,
                    steps: a.pos.get(4).map(|v| num(v, "steps")).transpose()?.unwrap_or(1.0),
                    width: a.opts.get("width").map(|v| num(v, "width")).transpose()?.unwrap_or(1),
                }
            }
            "shadow" => {
                exact(3, "shadow DX DY COLOUR")?;
                Cmd::Shadow(n(0)?, n(1)?, c(self, 2)?)
            }
            "lock" => Cmd::Lock(match args {
                [] | ["on"] => true,
                ["off"] => false,
                _ => return Err("usage: lock [on|off]".into()),
            }),
            "clip" => Cmd::Clip(match args {
                ["off"] => None,
                [_, _, _, _] => Some((n(0)?, n(1)?, n(2)?, n(3)?)),
                _ => return Err("usage: clip X Y W H | clip off".into()),
            }),
            "mode" => Cmd::Mode(match args {
                ["normal"] => Mode::Normal,
                ["over"] => Mode::Over,
                ["multiply"] => Mode::Multiply,
                ["add"] => Mode::Add,
                ["screen"] => Mode::Screen,
                _ => return Err("usage: mode normal|over|multiply|add|screen".into()),
            }),
            "scatter" => {
                let a = split_args(args, &["seed", "frame", "spacing"], &["surface", "vary"])?;
                if a.pos.len() != 6 {
                    return Err("usage: scatter NAME X Y W H COUNT [seed N] [frame F | vary] [surface] [spacing D]".into());
                }
                let name = a.pos[0];
                let r = self.known.get(name).ok_or(format!("`{name}` is not defined above (or in a `use`d file)"))?;
                let frame = if a.flags.contains(&"vary") { None } else { Some(a.opts.get("frame").map(|v| num(v, "frame")).transpose()?.unwrap_or(0)) };
                if frame.is_some_and(|f| f >= r.art_frames.len()) {
                    return Err(format!("`{name}` has {} frame(s)", r.art_frames.len()));
                }
                let p = |i: usize| num::<i32>(a.pos[i], cmd);
                Cmd::Scatter {
                    name: name.to_string(),
                    x: p(1)?,
                    y: p(2)?,
                    w: p(3)?,
                    h: p(4)?,
                    count: num(a.pos[5], "count")?,
                    seed: a.opts.get("seed").map(|v| num(v, "seed")).transpose()?.unwrap_or(1),
                    frame,
                    surface: a.flags.contains(&"surface"),
                    spacing: a.opts.get("spacing").map(|v| num(v, "spacing")).transpose()?.unwrap_or(0.0),
                }
            }
            "speckle" => {
                let a = split_args(args, &["seed"], &["anywhere"])?;
                if a.pos.len() != 6 {
                    return Err("usage: speckle X Y W H COLOUR DENSITY [seed N] [anywhere]".into());
                }
                let p = |i: usize| num::<i32>(a.pos[i], cmd);
                Cmd::Speckle {
                    x: p(0)?,
                    y: p(1)?,
                    w: p(2)?,
                    h: p(3)?,
                    c: self.color(a.pos[4])?,
                    density: num(a.pos[5], "density")?,
                    seed: a.opts.get("seed").map(|v| num(v, "seed")).transpose()?.unwrap_or(1),
                    anywhere: a.flags.contains(&"anywhere"),
                }
            }
            "shade" => {
                if args.is_empty() || args.len() > 2 {
                    return Err("usage: shade LIGHT-DIRECTION [STEPS]   e.g. shade tl".into());
                }
                Cmd::Shade { dir: direction(args[0])?, steps: if args.len() == 2 { f(1)? } else { 1.0 } }
            }
            "outline" => {
                let a = split_args(args, &[], &["diagonal", "inside"])?;
                if a.pos.len() != 1 {
                    return Err("usage: outline COLOUR [diagonal] [inside]".into());
                }
                Cmd::Outline { c: self.color(a.pos[0])?, diagonal: a.flags.contains(&"diagonal"), inside: a.flags.contains(&"inside") }
            }
            "glow" | "halo" => {
                if args.len() != 4 && args.len() != 5 {
                    return Err(format!("usage: {cmd} CX CY R COLOUR [STRENGTH]"));
                }
                let k = if args.len() == 5 { f(4)? } else { 1.0 };
                if cmd == "glow" {
                    Cmd::Glow(f(0)?, f(1)?, f(2)?, c(self, 3)?, k)
                } else {
                    Cmd::Halo(f(0)?, f(1)?, f(2)?, c(self, 3)?, k)
                }
            }
            "recolor" | "recolour" => {
                exact(2, "recolor FROM TO")?;
                Cmd::Recolor(c(self, 0)?, c(self, 1)?)
            }
            "mirror" | "flip" => {
                let v = match args {
                    ["x"] => false,
                    ["y"] => true,
                    _ => return Err(format!("usage: {cmd} x|y")),
                };
                if cmd == "mirror" {
                    Cmd::Mirror(v)
                } else {
                    Cmd::Flip(v)
                }
            }
            "shift" => {
                exact(2, "shift DX DY")?;
                Cmd::Shift(n(0)?, n(1)?)
            }
            "rotate" => {
                exact(1, "rotate 90|180|270")?;
                let q = match args[0] {
                    "90" => 1,
                    "180" => 2,
                    "270" => 3,
                    _ => return Err("usage: rotate 90|180|270".into()),
                };
                let p = self.need_cur(cmd)?;
                if q % 2 == 1 && p.w != p.h {
                    return Err("rotate 90/270 needs a square canvas".into());
                }
                Cmd::Rotate(q)
            }
            "stamp" => {
                let (name, frame, x, y) = match args {
                    [name, _, _] => (name, 0, n(1)?, n(2)?),
                    [name, fr, _, _] => (name, num(fr, "frame")?, n(2)?, n(3)?),
                    _ => return Err("usage: stamp NAME [FRAME] X Y".into()),
                };
                let r = self.known.get(*name).ok_or(format!("`{name}` is not defined above (or in a `use`d file)"))?;
                if frame >= r.art_frames.len() {
                    return Err(format!("`{name}` has {} frame(s)", r.art_frames.len()));
                }
                Cmd::Stamp { name: name.to_string(), frame, x, y }
            }
            _ => return Err(format!("unknown command `{cmd}`")),
        };
        self.need_cur(cmd)?.cmds().push((ln, draw));
        Ok(())
    }

    fn finish_resource(&mut self) -> Result<(), String> {
        let Some(p) = self.cur.take() else { return Ok(()) };
        let path = self.file.path.clone();
        let at = |ln: usize, msg: String| format!("{}:{ln}: {msg}", shown(&path));
        let mut base = Canvas::new(p.w, p.h);
        base.wrap = p.wrap;
        for (ln, c) in &p.base {
            self.run(&mut base, c, 0).map_err(|e| at(*ln, e))?;
        }
        let mut drawn: Vec<Canvas> = Vec::new();
        if p.frames.is_empty() {
            drawn.push(base);
        } else {
            for (fi, spec) in p.frames.iter().enumerate() {
                let mut cv = match spec.from {
                    Some(k) => drawn[k].clone(),
                    None => base.clone(),
                };
                cv.paint = Paint::default();
                for (ln, c) in &spec.cmds {
                    self.run(&mut cv, c, fi).map_err(|e| at(*ln, e))?;
                }
                drawn.push(cv);
            }
        }
        if let Some(fin) = &p.finish {
            for (fi, cv) in drawn.iter_mut().enumerate() {
                cv.paint = Paint::default();
                for (ln, c) in fin {
                    self.run(cv, c, fi).map_err(|e| at(*ln, e))?;
                }
            }
        }
        let frames: Vec<Canvas> = drawn
            .iter()
            .map(|c| {
                let mut c = if p.scale > 1 { c.upscale(p.scale) } else { c.clone() };
                c.wrap = false;
                c.paint = Paint::default();
                c
            })
            .collect();
        let s = p.scale;
        let origin = match p.origin {
            Origin::At(x, y) => (x * s, y * s),
            Origin::Center => (p.w * s / 2, p.h * s / 2),
            Origin::Bottom => (p.w * s / 2, p.h * s),
        };
        let r = Rc::new(Resource {
            kind: p.kind,
            name: p.name,
            file: path.clone(),
            line: p.line,
            w: p.w,
            h: p.h,
            scale: s,
            origin,
            mask: p.mask,
            bbox: p.bbox,
            tiles: p.tiles,
            wrap: p.wrap,
            icon: p.icon,
            against: p.against,
            art_frames: drawn,
            frames,
        });
        self.known.insert(r.name.clone(), r.clone());
        self.file.resources.push(r);
        Ok(())
    }

    fn run(&self, cv: &mut Canvas, cmd: &Cmd, frame_no: usize) -> Result<(), String> {
        match cmd {
            Cmd::Grid { x, y, cell: (cw, ch), rows } => {
                for (j, row) in rows.iter().enumerate() {
                    for (i, c) in row.iter().enumerate() {
                        let (px, py) = (x + i as i32 * cw, y + j as i32 * ch);
                        match c {
                            Cell::Leave => {}
                            Cell::Color(c) => cv.rect(px, py, *cw, *ch, *c),
                            Cell::Stamp(name, frame) => {
                                let r = self.known.get(name).ok_or(format!("`{name}` is not defined"))?;
                                cv.paste(&r.art_frames[*frame], px, py);
                            }
                        }
                    }
                }
            }
            Cmd::Pixel(x, y, c) => cv.put(*x, *y, *c),
            Cmd::Rect(x, y, w, h, c) => cv.rect(*x, *y, *w, *h, *c),
            Cmd::Box(x, y, w, h, c) => cv.frame_box(*x, *y, *w, *h, *c),
            Cmd::Line(a, b, c2, d, c) => cv.line(*a, *b, *c2, *d, *c),
            Cmd::Ellipse { cx, cy, rx, ry, angle, width, c } => cv.ellipse_ex(*cx, *cy, *rx, *ry, *angle, *width, *c),
            Cmd::Ring(x, y, r0, r1, c) => cv.ring(*x, *y, *r0, *r1, *c),
            Cmd::Poly(pts, c) => cv.poly(pts, *c),
            Cmd::Fill(x, y, c) => cv.flood(*x, *y, *c),
            Cmd::Gradient { x, y, w, h, stops, pos, dir, blend } => cv.gradient(*x, *y, *w, *h, stops, pos.as_deref(), *dir, *blend),
            Cmd::Noise { x, y, w, h, stops, pos, scale, seed, octaves, cut, blend } => {
                cv.noise(*x, *y, *w, *h, stops, pos.as_deref(), *scale, *seed, *octaves, *cut, *blend)
            }
            Cmd::Field { x, y, w, h, expr, stops, pos, cut, blend } => cv.field(*x, *y, *w, *h, expr, frame_no, stops, pos.as_deref(), *cut, *blend),
            Cmd::Speckle { x, y, w, h, c, density, seed, anywhere } => cv.speckle(*x, *y, *w, *h, *c, *density, *seed, *anywhere),
            Cmd::Ridge { x, y, w, h, stops, pos, scale, seed, octaves, min, blend } => cv.ridge(*x, *y, *w, *h, stops, pos.as_deref(), *scale, *seed, *octaves, *min, *blend),
            Cmd::Stripes { x, y, w, h, stops, widths, dir, offset } => cv.stripes(*x, *y, *w, *h, stops, widths, *dir, *offset),
            Cmd::Path(pts, c) => cv.path(pts, *c),
            Cmd::Bevel { x, y, w, h, steps, width } => cv.bevel(*x, *y, *w, *h, *steps, *width, 8.0),
            Cmd::Shadow(dx, dy, c) => cv.shadow(*dx, *dy, *c),
            Cmd::Lock(on) => cv.paint.lock = *on,
            Cmd::Clip(b) => cv.paint.clip = *b,
            Cmd::Mode(m) => cv.paint.mode = *m,
            Cmd::Scatter { name, x, y, w, h, count, seed, frame, surface, spacing } => {
                let r = self.known.get(name).ok_or(format!("`{name}` is not defined"))?;
                let (ox, oy) = (r.origin.0 / r.scale, r.origin.1 / r.scale);
                let mut placed: Vec<(f64, f64)> = Vec::new();
                for i in 0..*count as i32 {
                    let px = x + (hash(i, 1, *seed) * *w as f64) as i32;
                    let mut py = y + (hash(i, 2, *seed) * *h as f64) as i32;
                    if *surface {
                        match cv.surface(px, *y, y + h) {
                            Some(s) => py = s,
                            None => continue,
                        }
                    }
                    if placed.iter().any(|(qx, qy)| ((qx - px as f64).powi(2) + (qy - py as f64).powi(2)).sqrt() < *spacing) {
                        continue;
                    }
                    placed.push((px as f64, py as f64));
                    let f = frame.unwrap_or((hash(i, 3, *seed) * r.art_frames.len() as f64) as usize % r.art_frames.len());
                    cv.paste(&r.art_frames[f], px - ox, py - oy);
                }
            }
            Cmd::Shade { dir, steps } => cv.shade(*dir, *steps, 8.0),
            Cmd::Outline { c, diagonal, inside } => cv.outline(*c, *diagonal, *inside),
            Cmd::Glow(x, y, r, c, k) => cv.glow(*x, *y, *r, *c, *k),
            Cmd::Halo(x, y, r, c, k) => cv.halo(*x, *y, *r, *c, *k),
            Cmd::Recolor(a, b) => cv.recolor(*a, *b),
            Cmd::Mirror(v) => cv.mirror(*v),
            Cmd::Flip(v) => cv.flip(*v),
            Cmd::Shift(dx, dy) => cv.shift(*dx, *dy),
            Cmd::Rotate(q) => cv.rotate(*q),
            Cmd::Stamp { name, frame, x, y } => {
                let r = self.known.get(name).ok_or(format!("`{name}` is not defined"))?;
                cv.paste(&r.art_frames[*frame], *x, *y);
            }
        }
        Ok(())
    }
}

/// Every `.art` file under `dir` (recursively), sorted.
pub fn art_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "art") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Everything in a set of `.art` files: the resources (each once), warnings and the
/// strict palette, if one is declared.
#[derive(Default)]
pub struct ArtSet {
    pub resources: Vec<Rc<Resource>>,
    pub warnings: Vec<String>,
    pub palette: Option<Vec<Rgba>>,
}

pub fn load_set(files: &[PathBuf]) -> Result<ArtSet, String> {
    let mut loader = Loader::new();
    for f in files {
        loader.load(f)?;
    }
    let mut all: Vec<&Rc<ArtFile>> = loader.files().collect();
    all.sort_by(|a, b| a.path.cmp(&b.path));
    let mut set = ArtSet::default();
    let mut seen: HashMap<String, PathBuf> = HashMap::new();
    for f in all {
        set.warnings.extend(f.warnings.iter().cloned());
        if f.strict_palette {
            let mut cols: Vec<Rgba> = f.colors.values().copied().collect();
            cols.sort();
            set.palette.get_or_insert_with(Vec::new).extend(cols);
        }
        for r in &f.resources {
            if let Some(prev) = seen.insert(r.name.clone(), r.file.clone()) {
                return Err(format!("`{}` is defined in both {} and {}", r.name, shown(&prev), shown(&r.file)));
            }
            set.resources.push(r.clone());
        }
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<ArtFile, String> {
        Loader::new().parse(text, Path::new("test.art"))
    }

    #[test]
    fn grid_sprite_with_mirror_and_outline() {
        let f = parse(
            "color k #000\ncolor r #c82832\n\
             sprite spr_a 6x4\n  origin center\n\
             grid 1 0\n|.r|\n|rr|\n|rr|\n\
             mirror x\noutline k\n",
        )
        .unwrap();
        let r = &f.resources[0];
        assert_eq!((r.w, r.h, r.origin), (6, 4, (3, 2)));
        let c = &r.frames[0];
        assert_eq!(c.get(2, 0), rgb(200, 40, 50));
        assert_eq!(c.get(3, 0), rgb(200, 40, 50)); // mirrored
        assert_eq!(c.get(1, 0), rgb(0, 0, 0)); // outline
        assert_eq!(r.bbox_rect(), (0, 0, 5, 3));
    }

    #[test]
    fn frames_from_and_finish() {
        let f = parse(
            "sprite s 4x4\ncolor a #fff\npixel 0 0 a\n\
             frame\nframe from 0\npixel 1 1 a\nframe from 1\npixel 2 2 a\n\
             finish\nflip x\n",
        )
        .unwrap();
        let r = &f.resources[0];
        assert_eq!(r.frames.len(), 3);
        // frame 2 has all three pixels, flipped
        let c = &r.frames[2];
        assert!(c.opaque(3, 0) && c.opaque(2, 1) && c.opaque(1, 2));
        // `from` copies the frame before `finish` ran (it is not flipped twice)
        let c = &r.frames[1];
        assert!(c.opaque(3, 0) && c.opaque(2, 1) && !c.opaque(1, 2));
    }

    #[test]
    fn scale_multiplies_output() {
        let f = parse("sprite s 2x2\nscale 3\norigin 1 1\nrect 0 0 1 1 #fff\n").unwrap();
        let r = &f.resources[0];
        assert_eq!((r.frames[0].w, r.origin, r.bbox_rect()), (6, (3, 3), (0, 0, 2, 2)));
    }

    #[test]
    fn colour_modifiers_and_ramps() {
        let f = parse("ramp 123 #808080\nsprite s 3x1\npixel 0 0 1\npixel 1 0 2+0\npixel 2 0 #fff@50\n").unwrap();
        let c = &f.resources[0].frames[0];
        assert_eq!(c.get(1, 0), rgb(128, 128, 128));
        assert!(luminance(c.get(0, 0)) < luminance(c.get(1, 0)));
        assert_eq!(c.get(2, 0)[3], 128);
    }

    #[test]
    fn errors_name_the_line() {
        let e = parse("sprite s 4x4\nrect 0 0 2 two #fff\n").unwrap_err();
        assert!(e.contains("test.art:2:"), "{e}");
        let e = parse("sprite s 4x4\n|..z.\n").unwrap_err();
        assert!(e.contains("`z` is not defined"), "{e}");
    }

    #[test]
    fn ragged_grids_warn() {
        let f = parse("sprite s 4x4\ncolor a #fff\n|aaaa\n|aaa\n|aaaa\n").unwrap();
        assert_eq!(f.warnings.len(), 1, "{:?}", f.warnings);
        assert!(f.warnings[0].contains("line 4 is 3"), "{:?}", f.warnings);
    }

    #[test]
    fn paint_state_lock_clip_mode() {
        let f = parse(
            "sprite s 6x6
circle 3 3 2 #808080
             lock
rect 0 0 6 6 #ff0000
lock off
             clip 0 0 3 6
mode multiply
rect 0 0 6 6 #808080
",
        )
        .unwrap();
        let c = &f.resources[0].frames[0];
        assert_eq!(c.get(0, 0), CLEAR); // lock kept the clear corner clear
        assert_eq!(c.get(2, 3), rgb(128, 0, 0)); // red, multiplied by grey inside the clip
        assert_eq!(c.get(4, 3), rgb(255, 0, 0)); // outside the clip
    }

    #[test]
    fn colour_factor() {
        let f = parse("sprite s 1x1
pixel 0 0 #c86432*0.5@50
").unwrap();
        assert_eq!(f.resources[0].frames[0].get(0, 0), [100, 50, 25, 128]);
    }

    #[test]
    fn stripes_path_bevel_shadow() {
        let f = parse("sprite s 8x1
stripes 0 0 8 1 #000 #fff width 3 1 v
").unwrap();
        let c = &f.resources[0].frames[0];
        let row: Vec<u8> = (0..8).map(|x| c.get(x, 0)[0]).collect();
        assert_eq!(row, vec![0, 0, 0, 255, 0, 0, 0, 255]);

        let f = parse("sprite s 5x5
path #fff 0 0 4 0 4 4
").unwrap();
        let c = &f.resources[0].frames[0];
        assert!(c.opaque(2, 0) && c.opaque(4, 2) && !c.opaque(0, 2));

        let f = parse("sprite s 4x4
rect 0 0 4 4 #808080
bevel 0 0 4 4
").unwrap();
        let c = &f.resources[0].frames[0];
        assert!(luminance(c.get(0, 1)) > luminance(c.get(1, 1)) && luminance(c.get(3, 2)) < luminance(c.get(1, 1)));

        let f = parse("sprite s 4x4
pixel 0 0 #fff
shadow 1 1 #000
").unwrap();
        let c = &f.resources[0].frames[0];
        assert_eq!((c.get(0, 0), c.get(1, 1)), (rgb(255, 255, 255), rgb(0, 0, 0)));
    }

    #[test]
    fn grid_cells_stamp_pieces() {
        let f = parse(
            "piece brick 2x2
rect 0 0 2 2 #f00
             sprite s 6x2
tile B brick
color g #0f0
grid 0 0 cell 2
|B.g
",
        )
        .unwrap();
        let c = &f.resources[1].frames[0];
        assert_eq!((c.get(1, 1), c.get(2, 0), c.get(5, 1)), (rgb(255, 0, 0), CLEAR, rgb(0, 255, 0)));
    }

    #[test]
    fn ellipse_ring_ridge_scatter_band() {
        let f = parse("sprite s 21x21
ellipse 10.5 10.5 10 4 #fff angle 90 width 1
").unwrap();
        let c = &f.resources[0].frames[0];
        assert!(c.opaque(10, 0) && !c.opaque(10, 10) && !c.opaque(0, 10));

        let f = parse("background b 32x16
wrap
ridge 0 0 32 16 #fff scale 8 min 0.25
").unwrap();
        let c = &f.resources[0].frames[0];
        assert!((0..32).all(|x| c.opaque(x, 15) && c.opaque(x, 12)));

        let f = parse(
            "piece dot 1x1
pixel 0 0 #f00
             sprite s 16x16
rect 0 10 16 6 #00f
scatter dot 0 0 16 16 5 surface
",
        )
        .unwrap();
        let c = &f.resources[1].frames[0];
        assert!((0..16).filter(|x| c.get(*x, 10) == rgb(255, 0, 0)).count() >= 3);

        let f = parse("sprite s 32x32
noise 0 0 32 32 #fff band 0.45 0.55 scale 8
").unwrap();
        let n = f.resources[0].frames[0].px.iter().filter(|p| p[3] > 0).count();
        assert!(n > 20 && n < 600, "{n}");
    }

    #[test]
    fn field_and_positioned_stops() {
        let f = parse("sprite s 4x1\nfield 0 0 4 1 \"u\" #000 #fff:0.25 #fff\n").unwrap();
        let c = &f.resources[0].frames[0];
        // u = 0.125, 0.375, ... : the first pixel is half way to white, the rest white
        assert_eq!(c.get(0, 0), rgb(128, 128, 128));
        assert_eq!(c.get(2, 0), rgb(255, 255, 255));
        let f = parse("sprite s 2x1\nframe\nframe\nfinish\nfield 0 0 2 1 \"f\" #000 #fff\n").unwrap();
        let r = &f.resources[0];
        assert_eq!((r.frames[0].get(0, 0), r.frames[1].get(0, 0)), (rgb(0, 0, 0), rgb(255, 255, 255)));
        assert!(parse("sprite s 2x1\nfield 0 0 2 1 \"sin(\" #000 #fff\n").unwrap_err().contains("field"));
    }

    #[test]
    fn stamp_uses_pieces() {
        let f = parse("piece dot 1x1\npixel 0 0 #f00\nsprite s 3x3\nstamp dot 2 2\n").unwrap();
        assert_eq!(f.resources[1].frames[0].get(2, 2), rgb(255, 0, 0));
    }
}
