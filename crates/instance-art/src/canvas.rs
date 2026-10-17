//! An RGBA pixel canvas and the drawing operations of the `.art` format.

use crate::color::*;
use instance_format::RgbaImage;

#[derive(Clone, Debug, PartialEq)]
pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub px: Vec<Rgba>,
    /// Drawing wraps around the edges (for seamless textures).
    pub wrap: bool,
    /// How painting commands change pixels (`lock`, `clip`, `mode`).
    pub paint: Paint,
}

/// How a painted colour combines with the pixel under it.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Mode {
    /// Replaces the pixel (stamps are drawn over it).
    #[default]
    Normal,
    /// Alpha-blends over the pixel.
    Over,
    /// Multiplies the pixel's colour (darkens); the painted alpha is the strength.
    Multiply,
    /// Adds to the pixel's colour (light).
    Add,
    /// Screens the pixel's colour (lightens softly).
    Screen,
}

/// Painting state: set by `lock`, `clip` and `mode`, kept from command to command.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Paint {
    /// Only pixels that are already opaque change, and they keep their alpha.
    pub lock: bool,
    /// Only pixels inside (x, y, w, h) change.
    pub clip: Option<(i32, i32, i32, i32)>,
    pub mode: Mode,
}

/// How a gradient or noise value picks between colour stops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Blend {
    /// Mixes neighbouring stops.
    Smooth,
    /// Hard bands of the stops themselves.
    Bands,
    /// Ordered (Bayer 4x4) dithering between neighbouring stops.
    Dither,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Direction {
    Vertical,
    Horizontal,
    Radial,
}

const BAYER4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// A hash of a pixel position, in [0, 1).
pub fn hash(x: i32, y: i32, seed: u32) -> f64 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    (h ^ (h >> 16)) as f64 / u32::MAX as f64
}

/// Smooth value noise in [0, 1); lattice coordinates wrap with period (px, py) when > 0.
pub fn vnoise(x: f64, y: f64, seed: u32, px: i32, py: i32) -> f64 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let s = |t: f64| t * t * (3.0 - 2.0 * t);
    let (u, v) = (s(x - x.floor()), s(y - y.floor()));
    let wx = |i: i32| if px > 0 { i.rem_euclid(px) } else { i };
    let wy = |i: i32| if py > 0 { i.rem_euclid(py) } else { i };
    let a = hash(wx(xi), wy(yi), seed);
    let b = hash(wx(xi + 1), wy(yi), seed);
    let c = hash(wx(xi), wy(yi + 1), seed);
    let d = hash(wx(xi + 1), wy(yi + 1), seed);
    a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

/// Picks a colour for `t` in [0, 1] among `stops`; `(x, y)` places the dither pattern.
/// With `pos` (one position per stop, ascending), the stops sit at those positions
/// instead of being evenly spaced.
pub fn pick(stops: &[Rgba], pos: Option<&[f64]>, t: f64, blend: Blend, x: i32, y: i32) -> Rgba {
    let n = stops.len();
    if n == 1 {
        return stops[0];
    }
    if let Some(pos) = pos {
        if t <= pos[0] {
            return stops[0];
        }
        if t >= pos[n - 1] {
            return stops[n - 1];
        }
        let i = (0..n - 1).find(|&i| t < pos[i + 1]).unwrap_or(n - 2);
        if blend == Blend::Bands {
            return stops[i];
        }
        let span = pos[i + 1] - pos[i];
        let frac = if span > 0.0 { (t - pos[i]) / span } else { 1.0 };
        return if blend == Blend::Smooth {
            mix(stops[i], stops[i + 1], frac)
        } else {
            let threshold = (BAYER4[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize] as f64 + 0.5) / 16.0;
            if frac > threshold {
                stops[i + 1]
            } else {
                stops[i]
            }
        };
    }
    let t = t.clamp(0.0, 1.0);
    match blend {
        Blend::Bands => stops[((t * n as f64) as usize).min(n - 1)],
        Blend::Smooth | Blend::Dither => {
            let f = t * (n - 1) as f64;
            let i = (f.floor() as usize).min(n - 2);
            let frac = f - i as f64;
            if blend == Blend::Smooth {
                mix(stops[i], stops[i + 1], frac)
            } else {
                let threshold = (BAYER4[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize] as f64 + 0.5) / 16.0;
                if frac > threshold {
                    stops[i + 1]
                } else {
                    stops[i]
                }
            }
        }
    }
}

const DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// The pixels of a 1-pixel line (Bresenham), both ends included.
pub fn line_points(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    let mut out = Vec::new();
    loop {
        out.push((x, y));
        if x == x1 && y == y1 {
            return out;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Self {
        Canvas { w, h, px: vec![CLEAR; (w.max(0) * h.max(0)) as usize], wrap: false, paint: Paint::default() }
    }

    pub fn from_image(img: &RgbaImage) -> Self {
        let mut c = Canvas::new(img.width as i32, img.height as i32);
        for (i, p) in c.px.iter_mut().enumerate() {
            p.copy_from_slice(&img.data[i * 4..i * 4 + 4]);
        }
        c
    }

    pub fn image(&self) -> RgbaImage {
        let mut img = RgbaImage::new(self.w as u32, self.h as u32);
        for (i, c) in self.px.iter().enumerate() {
            img.data[i * 4..i * 4 + 4].copy_from_slice(c);
        }
        img
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if self.w <= 0 || self.h <= 0 {
            return None;
        }
        let (x, y) = if self.wrap { (x.rem_euclid(self.w), y.rem_euclid(self.h)) } else { (x, y) };
        (x >= 0 && y >= 0 && x < self.w && y < self.h).then(|| (y * self.w + x) as usize)
    }

    pub fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if let Some(i) = self.index(x, y) {
            self.px[i] = c;
        }
    }

    pub fn get(&self, x: i32, y: i32) -> Rgba {
        self.index(x, y).map(|i| self.px[i]).unwrap_or(CLEAR)
    }

    /// Draws `c` over the pixel with alpha blending.
    pub fn blend(&mut self, x: i32, y: i32, c: Rgba) {
        if let Some(i) = self.index(x, y) {
            self.px[i] = over(c, self.px[i]);
        }
    }

    pub fn opaque(&self, x: i32, y: i32) -> bool {
        self.get(x, y)[3] > 0
    }

    fn in_clip(&self, x: i32, y: i32) -> bool {
        match self.paint.clip {
            Some((cx, cy, cw, ch)) => {
                let (x, y) = if self.wrap { (x.rem_euclid(self.w.max(1)), y.rem_euclid(self.h.max(1))) } else { (x, y) };
                x >= cx && y >= cy && x < cx + cw && y < cy + ch
            }
            None => true,
        }
    }

    /// Sets a pixel inside the clip box (effects: shade, outline, glow, ...).
    pub fn set_clipped(&mut self, x: i32, y: i32, c: Rgba) {
        if self.in_clip(x, y) {
            self.set(x, y, c);
        }
    }

    /// Paints a pixel through the painting state (clip, lock, mode).
    pub fn put(&mut self, x: i32, y: i32, c: Rgba) {
        self.put_as(x, y, c, self.paint.mode);
    }

    /// Paints a stamped pixel: like `put`, but `normal` mode draws it over the pixel.
    pub fn put_stamp(&mut self, x: i32, y: i32, c: Rgba) {
        let mode = if self.paint.mode == Mode::Normal { Mode::Over } else { self.paint.mode };
        self.put_as(x, y, c, mode);
    }

    fn put_as(&mut self, x: i32, y: i32, c: Rgba, mode: Mode) {
        if !self.in_clip(x, y) {
            return;
        }
        let Some(i) = self.index(x, y) else { return };
        let old = self.px[i];
        if self.paint.lock && old[3] == 0 {
            return;
        }
        let a = c[3] as f64 / 255.0;
        let ch = |f: &dyn Fn(f64, f64) -> f64| -> Rgba {
            let g = |k: usize| {
                let (o, n) = (old[k] as f64, c[k] as f64);
                (o + (f(o, n) - o) * a).round().clamp(0.0, 255.0) as u8
            };
            [g(0), g(1), g(2), old[3]]
        };
        let mut new = match mode {
            Mode::Normal => c,
            Mode::Over => over(c, old),
            // these change existing colour; a clear pixel has none
            _ if old[3] == 0 => return,
            Mode::Multiply => ch(&|o, n| o * n / 255.0),
            Mode::Add => ch(&|o, n| o + n),
            Mode::Screen => ch(&|o, n| 255.0 - (255.0 - o) * (255.0 - n) / 255.0),
        };
        if self.paint.lock {
            new[3] = old[3];
        }
        self.px[i] = new;
    }

    /// Calls `f` with the centre of every pixel that may lie in (x0, y0)-(x1, y1);
    /// `Some(colour)` paints it.
    fn fill_box(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, mut f: impl FnMut(f64, f64) -> Option<Rgba>) {
        let lo = |v: f64, n: i32| (v.floor() as i64 - 1).clamp(0, n as i64) as i32;
        let (xa, xb, ya, yb) = (lo(x0, self.w), lo(x1 + 2.0, self.w), lo(y0, self.h), lo(y1 + 2.0, self.h));
        for y in ya..yb {
            for x in xa..xb {
                if let Some(c) = f(x as f64 + 0.5, y as f64 + 0.5) {
                    self.put(x, y, c);
                }
            }
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, c);
            }
        }
    }

    pub fn frame_box(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        if w <= 0 || h <= 0 {
            return;
        }
        let mut pts = Vec::new();
        for xx in x..x + w {
            pts.push((xx, y));
            pts.push((xx, y + h - 1));
        }
        for yy in y + 1..y + h - 1 {
            pts.push((x, yy));
            pts.push((x + w - 1, yy));
        }
        pts.sort();
        pts.dedup();
        for (px, py) in pts {
            self.put(px, py, c);
        }
    }

    /// A 1-pixel line between two pixels (Bresenham), both ends included.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgba) {
        for (x, y) in line_points(x0, y0, x1, y1) {
            self.put(x, y, c);
        }
    }

    /// Connected lines through the points (each joint painted once).
    pub fn path(&mut self, pts: &[(i32, i32)], c: Rgba) {
        let mut all: Vec<(i32, i32)> = Vec::new();
        for w in pts.windows(2) {
            for p in line_points(w[0].0, w[0].1, w[1].0, w[1].1) {
                if all.last() != Some(&p) {
                    all.push(p);
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        for p in all {
            if seen.insert(p) {
                self.put(p.0, p.1, c);
            }
        }
    }

    /// A filled ellipse, rotated by `angle` degrees; with `width`, only a band that
    /// wide inside its edge (a ring).
    #[allow(clippy::too_many_arguments)]
    pub fn ellipse_ex(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, angle: f64, width: Option<f64>, c: Rgba) {
        let (s, co) = angle.to_radians().sin_cos();
        let r = rx.max(ry);
        self.fill_box(cx - r, cy - r, cx + r, cy + r, |x, y| {
            let (dx, dy) = (x - cx, y - cy);
            let (u, v) = (dx * co + dy * s, -dx * s + dy * co);
            let e = (u / rx).powi(2) + (v / ry).powi(2);
            if e > 1.0 {
                return None;
            }
            match width {
                Some(wd) if rx > wd && ry > wd => (((u / (rx - wd)).powi(2) + (v / (ry - wd)).powi(2)) > 1.0).then_some(c),
                _ => Some(c),
            }
        });
    }

    pub fn ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, c: Rgba) {
        self.ellipse_ex(cx, cy, rx, ry, 0.0, None, c);
    }

    pub fn ring(&mut self, cx: f64, cy: f64, r0: f64, r1: f64, c: Rgba) {
        self.fill_box(cx - r1, cy - r1, cx + r1, cy + r1, |x, y| {
            let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
            (d >= r0 && d <= r1).then_some(c)
        });
    }

    /// Fills the pixels whose centres are inside the polygon (even-odd rule).
    pub fn poly(&mut self, pts: &[(f64, f64)], c: Rgba) {
        let fold = |f: fn(f64, f64) -> f64, init: f64, k: usize| pts.iter().map(|p| if k == 0 { p.0 } else { p.1 }).fold(init, f);
        let (x0, x1) = (fold(f64::min, f64::INFINITY, 0), fold(f64::max, f64::NEG_INFINITY, 0));
        let (y0, y1) = (fold(f64::min, f64::INFINITY, 1), fold(f64::max, f64::NEG_INFINITY, 1));
        self.fill_box(x0, y0, x1, y1, |x, y| {
            let mut inside = false;
            let n = pts.len();
            for i in 0..n {
                let (ax, ay) = pts[i];
                let (bx, by) = pts[(i + n - 1) % n];
                if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
                    inside = !inside;
                }
            }
            inside.then_some(c)
        });
    }

    /// Flood-fills the 4-connected region of the colour at (x, y).
    pub fn flood(&mut self, x: i32, y: i32, c: Rgba) {
        let Some(start) = self.index(x, y) else { return };
        let target = self.px[start];
        let mut region = Vec::new();
        let mut seen = vec![false; self.px.len()];
        let mut stack = vec![(x, y)];
        while let Some((x, y)) = stack.pop() {
            match self.index(x, y) {
                Some(i) if !seen[i] && self.px[i] == target => {
                    seen[i] = true;
                    region.push((x, y));
                    for (dx, dy) in DIRS {
                        let (nx, ny) = (x + dx, y + dy);
                        if self.wrap || (nx >= 0 && ny >= 0 && nx < self.w && ny < self.h) {
                            stack.push((nx, ny));
                        }
                    }
                }
                _ => {}
            }
        }
        for (x, y) in region {
            self.put(x, y, c);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn gradient(&mut self, x: i32, y: i32, w: i32, h: i32, stops: &[Rgba], pos: Option<&[f64]>, dir: Direction, blend: Blend) {
        for yy in y..y + h {
            for xx in x..x + w {
                let (fx, fy) = ((xx - x) as f64 + 0.5, (yy - y) as f64 + 0.5);
                let t = match dir {
                    Direction::Vertical => fy / h as f64,
                    Direction::Horizontal => fx / w as f64,
                    Direction::Radial => {
                        let (dx, dy) = ((fx - w as f64 / 2.0) / (w as f64 / 2.0), (fy - h as f64 / 2.0) / (h as f64 / 2.0));
                        (dx * dx + dy * dy).sqrt()
                    }
                };
                self.put(xx, yy, pick(stops, pos, t, blend, xx, yy));
            }
        }
    }

    /// Paints a box from an expression of the pixel (see [`crate::expr`]), its value
    /// picked among the stops (or, with `cut`, painted with the last stop where the
    /// value is in (lo, hi]).
    #[allow(clippy::too_many_arguments)]
    pub fn field(&mut self, x: i32, y: i32, w: i32, h: i32, e: &crate::expr::Expr, frame: usize, stops: &[Rgba], pos: Option<&[f64]>, cut: Option<(f64, f64)>, blend: Blend) {
        for yy in y..y + h {
            for xx in x..x + w {
                let (fx, fy) = (xx as f64 + 0.5, yy as f64 + 0.5);
                let vars = [fx, fy, (fx - x as f64) / w as f64, (fy - y as f64) / h as f64, w as f64, h as f64, frame as f64];
                let v = crate::expr::eval(e, &vars);
                if !v.is_finite() {
                    continue;
                }
                match cut {
                    Some((lo, hi)) => {
                        if v > lo && v <= hi {
                            self.put(xx, yy, *stops.last().unwrap());
                        }
                    }
                    None => self.put(xx, yy, pick(stops, pos, v, blend, xx, yy)),
                }
            }
        }
    }

    /// Fractal value noise in 0..1 at a pixel (periodic over the canvas with `wrap`).
    fn noise_at(&self, xx: f64, yy: f64, scale: f64, seed: u32, octaves: u32) -> f64 {
        // when wrapping, round the scale so a whole number of cells fits the canvas
        let (mut sx, mut sy, mut px, mut py) = (scale, scale, 0, 0);
        if self.wrap {
            px = ((self.w as f64 / scale).round() as i32).max(1);
            py = ((self.h as f64 / scale).round() as i32).max(1);
            sx = self.w as f64 / px as f64;
            sy = self.h as f64 / py as f64;
        }
        let (mut sum, mut amp, mut f, mut norm) = (0.0, 0.5, 1.0, 0.0);
        for o in 0..octaves.max(1) {
            let k = 1 << o;
            sum += amp * vnoise(xx / sx * f, yy / sy * f, seed.wrapping_add(o * 17), px * k, py * k);
            norm += amp;
            amp *= 0.5;
            f *= 2.0;
        }
        // stretch the fractal sum (which bunches around 0.5) to fill 0..1
        ((sum / norm - 0.5) * 1.8 + 0.5).clamp(0.0, 1.0)
    }

    /// Fractal value noise over a box. With `cut` (lo, hi), only pixels whose value is
    /// between them are painted (with the last stop), for spots, veins and lines.
    #[allow(clippy::too_many_arguments)]
    pub fn noise(&mut self, x: i32, y: i32, w: i32, h: i32, stops: &[Rgba], pos: Option<&[f64]>, scale: f64, seed: u32, octaves: u32, cut: Option<(f64, f64)>, blend: Blend) {
        for yy in y..y + h {
            for xx in x..x + w {
                let v = self.noise_at(xx as f64 + 0.5, yy as f64 + 0.5, scale, seed, octaves);
                match cut {
                    Some((lo, hi)) => {
                        if v > lo && v <= hi {
                            self.put(xx, yy, *stops.last().unwrap());
                        }
                    }
                    None => self.put(xx, yy, pick(stops, pos, v, blend, xx, yy)),
                }
            }
        }
    }

    /// Columns of a box filled from a noisy top edge down to the bottom: hills,
    /// mountains, skylines. The edge is at `min`..1 of the box height from the bottom;
    /// the stops run from the edge (first) to the bottom of the box (last).
    #[allow(clippy::too_many_arguments)]
    pub fn ridge(&mut self, x: i32, y: i32, w: i32, h: i32, stops: &[Rgba], pos: Option<&[f64]>, scale: f64, seed: u32, octaves: u32, min: f64, blend: Blend) {
        for xx in x..x + w {
            let v = self.noise_at(xx as f64 + 0.5, 0.5, scale, seed, octaves);
            let height = h as f64 * (min + (1.0 - min) * v);
            let top = y as f64 + h as f64 - height;
            for yy in y..y + h {
                let fy = yy as f64 + 0.5;
                if fy >= top {
                    let t = if height > 0.0 { (fy - top) / height } else { 0.0 };
                    self.put(xx, yy, pick(stops, pos, t, blend, xx, yy));
                }
            }
        }
    }

    /// Bands of colour: `widths[i]` pixels of `stops[i]`, repeating, across a box.
    /// The bands run horizontally (`h`), vertically (`v`), diagonally `/` (`d`) or `\`
    /// (`a`), measured from the canvas origin so neighbouring boxes line up.
    #[allow(clippy::too_many_arguments)]
    pub fn stripes(&mut self, x: i32, y: i32, w: i32, h: i32, stops: &[Rgba], widths: &[i32], dir: char, offset: i32) {
        let period: i32 = widths.iter().sum::<i32>().max(1);
        for yy in y..y + h {
            for xx in x..x + w {
                let p = match dir {
                    'h' => yy,
                    'v' => xx,
                    'd' => xx + yy,
                    _ => xx - yy,
                } + offset;
                let mut r = p.rem_euclid(period);
                let mut i = 0;
                while r >= widths[i] {
                    r -= widths[i];
                    i += 1;
                }
                self.put(xx, yy, stops[i % stops.len()]);
            }
        }
    }

    /// Random single pixels of `c` at `density` (0..1) inside a box; only on opaque
    /// pixels unless `anywhere`.
    #[allow(clippy::too_many_arguments)]
    pub fn speckle(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba, density: f64, seed: u32, anywhere: bool) {
        for yy in y..y + h {
            for xx in x..x + w {
                if hash(xx, yy, seed ^ 0x5bd1_e995) < density && (anywhere || self.opaque(xx, yy)) {
                    self.put(xx, yy, c);
                }
            }
        }
    }

    /// Lights the top and left `width` pixels of a box and shades the bottom and right
    /// ones by `steps` ramp steps: bricks, tiles, buttons.
    #[allow(clippy::too_many_arguments)]
    pub fn bevel(&mut self, x: i32, y: i32, w: i32, h: i32, steps: f64, width: i32, hue: f64) {
        let src = self.clone();
        for yy in y..y + h {
            for xx in x..x + w {
                let c = src.get(xx, yy);
                if c[3] == 0 {
                    continue;
                }
                let (l, t, r, b) = (xx - x, yy - y, x + w - 1 - xx, y + h - 1 - yy);
                let lit = l.min(t) < width;
                let dark = r.min(b) < width;
                if lit && !(dark && r.min(b) < l.min(t)) {
                    self.set_clipped(xx, yy, step(c, steps, hue));
                } else if dark {
                    self.set_clipped(xx, yy, step(c, -steps, hue));
                }
            }
        }
    }

    /// The image's silhouette in `c`, offset by (dx, dy), drawn behind it.
    pub fn shadow(&mut self, dx: i32, dy: i32, c: Rgba) {
        let src = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                let s = src.get(x - dx, y - dy);
                if s[3] > 0 && self.in_clip(x, y) {
                    let a = (c[3] as u32 * s[3] as u32 / 255) as u8;
                    let p = src.get(x, y);
                    self.set(x, y, over(p, [c[0], c[1], c[2], a]));
                }
            }
        }
    }

    /// Rim lighting: opaque pixels with a transparent neighbour towards the light get
    /// lighter, those with one on the far side get darker (hue-shifted).
    pub fn shade(&mut self, (lx, ly): (i32, i32), steps: f64, hue: f64) {
        let src = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                let c = src.get(x, y);
                if c[3] == 0 {
                    continue;
                }
                let lit = (lx != 0 && !src.opaque(x + lx, y)) || (ly != 0 && !src.opaque(x, y + ly));
                let dark = (lx != 0 && !src.opaque(x - lx, y)) || (ly != 0 && !src.opaque(x, y - ly));
                if lit && !dark {
                    self.set_clipped(x, y, step(c, steps, hue));
                } else if dark && !lit {
                    self.set_clipped(x, y, step(c, -steps, hue));
                }
            }
        }
    }

    /// Paints the transparent pixels next to opaque ones (`inside`: recolours the
    /// opaque pixels next to transparent ones instead). `diagonal` also counts corners.
    pub fn outline(&mut self, c: Rgba, diagonal: bool, inside: bool) {
        let src = self.clone();
        let mut dirs = DIRS.to_vec();
        if diagonal {
            dirs.extend([(1, 1), (1, -1), (-1, 1), (-1, -1)]);
        }
        for y in 0..self.h {
            for x in 0..self.w {
                let me = src.opaque(x, y);
                if me == inside && dirs.iter().any(|(dx, dy)| src.opaque(x + dx, y + dy) != inside) {
                    self.set_clipped(x, y, c);
                }
            }
        }
    }

    /// Adds light around a point, on opaque pixels only.
    pub fn glow(&mut self, cx: f64, cy: f64, r: f64, light: Rgba, k: f64) {
        for y in 0..self.h {
            for x in 0..self.w {
                let d = ((x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2)).sqrt();
                let p = self.get(x, y);
                if d < r && p[3] > 0 {
                    self.set_clipped(x, y, add_light(p, light, (1.0 - d / r).powi(2) * k));
                }
            }
        }
    }

    /// A soft translucent disc of `c` behind the existing pixels (glows, shadows).
    pub fn halo(&mut self, cx: f64, cy: f64, r: f64, c: Rgba, k: f64) {
        for y in 0..self.h {
            for x in 0..self.w {
                let d = ((x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2)).sqrt();
                if d < r {
                    let a = ((1.0 - d / r).powi(2) * k * c[3] as f64).round().clamp(0.0, 255.0) as u8;
                    let p = self.get(x, y);
                    self.set_clipped(x, y, over(p, [c[0], c[1], c[2], a]));
                }
            }
        }
    }

    pub fn recolor(&mut self, from: Rgba, to: Rgba) {
        for y in 0..self.h {
            for x in 0..self.w {
                if self.get(x, y) == from {
                    self.set_clipped(x, y, to);
                }
            }
        }
    }

    /// Copies the left half onto the right half, mirrored (`vertical`: top onto bottom).
    pub fn mirror(&mut self, vertical: bool) {
        for y in 0..self.h {
            for x in 0..self.w {
                if vertical && y >= (self.h + 1) / 2 {
                    let c = self.get(x, self.h - 1 - y);
                    self.set(x, y, c);
                } else if !vertical && x >= (self.w + 1) / 2 {
                    let c = self.get(self.w - 1 - x, y);
                    self.set(x, y, c);
                }
            }
        }
    }

    pub fn flip(&mut self, vertical: bool) {
        let src = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                let c = if vertical { src.get(x, self.h - 1 - y) } else { src.get(self.w - 1 - x, y) };
                self.set(x, y, c);
            }
        }
    }

    /// Moves the image; uncovered pixels become clear (or wrap around with `wrap`).
    pub fn shift(&mut self, dx: i32, dy: i32) {
        let src = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                let c = src.get(x - dx, y - dy);
                self.px[(y * self.w + x) as usize] = c;
            }
        }
    }

    /// Rotates by a multiple of 90 degrees clockwise (square canvases only for 90/270).
    pub fn rotate(&mut self, quarter_turns: i32) {
        for _ in 0..quarter_turns.rem_euclid(4) {
            let src = self.clone();
            for y in 0..self.h {
                for x in 0..self.w {
                    self.px[(y * self.w + x) as usize] = src.get(y, src.h - 1 - x);
                }
            }
        }
    }

    /// Draws `src` at (ox, oy) over this canvas (alpha blended; clear pixels leave
    /// what is below), through the painting state.
    pub fn paste(&mut self, src: &Canvas, ox: i32, oy: i32) {
        for y in 0..src.h {
            for x in 0..src.w {
                let c = src.get(x, y);
                if c[3] > 0 {
                    self.put_stamp(ox + x, oy + y, c);
                }
            }
        }
    }

    /// The first opaque pixel at or below `y0` in column `x` (up to `y1`, exclusive).
    pub fn surface(&self, x: i32, y0: i32, y1: i32) -> Option<i32> {
        (y0..y1).find(|&y| self.opaque(x, y))
    }

    /// Each pixel becomes an `n`x`n` block.
    pub fn upscale(&self, n: i32) -> Canvas {
        let mut out = Canvas::new(self.w * n, self.h * n);
        for y in 0..out.h {
            for x in 0..out.w {
                out.px[(y * out.w + x) as usize] = self.px[((y / n) * self.w + x / n) as usize];
            }
        }
        out
    }

    /// The box (left, top, right, bottom) of pixels with alpha above `tolerance`.
    pub fn opaque_box(&self, tolerance: u8) -> Option<(i32, i32, i32, i32)> {
        let (mut l, mut t, mut r, mut b) = (i32::MAX, i32::MAX, -1, -1);
        for y in 0..self.h {
            for x in 0..self.w {
                if self.px[(y * self.w + x) as usize][3] > tolerance {
                    l = l.min(x);
                    r = r.max(x);
                    t = t.min(y);
                    b = b.max(y);
                }
            }
        }
        (r >= 0).then_some((l, t, r, b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const K: Rgba = rgb(0, 0, 0);

    #[test]
    fn line_and_box() {
        let mut c = Canvas::new(5, 5);
        c.line(0, 0, 4, 4, K);
        assert!((0..5).all(|i| c.opaque(i, i)));
        let mut c = Canvas::new(4, 4);
        c.frame_box(0, 0, 4, 4, K);
        assert!(c.opaque(0, 3) && c.opaque(3, 0) && !c.opaque(1, 1));
    }

    #[test]
    fn circle_is_symmetric() {
        let mut c = Canvas::new(16, 16);
        c.ellipse(8.0, 8.0, 5.0, 5.0, K);
        let mut m = c.clone();
        m.flip(false);
        assert_eq!(c, m);
        assert!(c.opaque(3, 8) && !c.opaque(2, 8) && c.opaque(12, 8) && !c.opaque(13, 8));
    }

    #[test]
    fn wrap_noise_is_seamless() {
        let mut c = Canvas::new(32, 32);
        c.wrap = true;
        c.noise(0, 0, 32, 32, &[rgb(0, 0, 0), rgb(255, 255, 255)], None, 8.0, 3, 3, None, Blend::Smooth);
        let seam: f64 = (0..32).map(|y| distance(c.get(31, y), c.get(0, y))).sum::<f64>() / 32.0;
        let inner: f64 = (0..32).map(|y| distance(c.get(15, y), c.get(16, y))).sum::<f64>() / 32.0;
        assert!(seam < inner * 2.5 + 0.01, "seam {seam} inner {inner}");
    }

    #[test]
    fn outline_and_flood() {
        let mut c = Canvas::new(5, 5);
        c.set(2, 2, K);
        c.outline(rgb(255, 0, 0), false, false);
        assert_eq!(c.get(2, 1), rgb(255, 0, 0));
        assert_eq!(c.get(1, 1), CLEAR);
        c.flood(0, 0, rgb(0, 255, 0));
        assert_eq!(c.get(1, 1), rgb(0, 255, 0));
        assert_eq!(c.get(2, 2), K);
    }
}
