//! Contact sheets: every sprite and background zoomed in, against the game's backdrop,
//! light and dark, as a silhouette with its mask and origin, at real size, and (in
//! detail mode) frame-to-frame motion and tile seams.

use crate::canvas::Canvas;
use crate::color::*;
use crate::font::*;
use crate::game::{Item, Scene};

const PAGE_BG: Rgba = rgb(30, 30, 34);
const CARD_BG: Rgba = rgb(44, 44, 50);
const TEXT: Rgba = rgb(240, 240, 240);
const SUBTEXT: Rgba = rgb(160, 160, 170);
const LIGHT: Rgba = rgb(232, 232, 232);
const DARK: Rgba = rgb(20, 20, 20);
const MASK: Rgba = rgb(230, 40, 40);
const ORIGIN: Rgba = rgb(240, 40, 220);
const GAP: i32 = 8;
const PAD: i32 = 10;

pub struct Options {
    /// Fixed zoom; `None` picks one per item.
    pub zoom: Option<i32>,
    /// Frame-to-frame motion and larger zoom for every item.
    pub detail: bool,
    /// Page width in pixels.
    pub width: i32,
    /// Start a new page past this height.
    pub page_height: i32,
}

impl Default for Options {
    fn default() -> Self {
        Options { zoom: None, detail: false, width: 1400, page_height: 1800 }
    }
}

/// `num/den` scaling: a zoom of `num` (or a reduction by `den`).
#[derive(Clone, Copy)]
struct Zoom {
    num: i32,
    den: i32,
}

impl Zoom {
    fn px(self, v: i32) -> i32 {
        v * self.num / self.den
    }
    fn label(self) -> String {
        if self.den == 1 {
            format!("{}×", self.num)
        } else {
            format!("1/{}", self.den)
        }
    }
}

fn pick_zoom(w: i32, h: i32, target: i32, max: i32, fixed: Option<i32>) -> Zoom {
    if let Some(z) = fixed {
        return Zoom { num: z.max(1), den: 1 };
    }
    let m = w.max(h).max(1);
    if m > max {
        return Zoom { num: 1, den: (m + max - 1) / max };
    }
    Zoom { num: (target / m).clamp(1, 8), den: 1 }
}

/// `src` scaled by `z`, drawn over a backdrop function, with `pad` pixels around it.
fn panel(src: &Canvas, z: Zoom, pad: i32, back: &dyn Fn(i32, i32) -> Rgba) -> Canvas {
    let (w, h) = (z.px(src.w), z.px(src.h));
    let mut out = Canvas::new(w + 2 * pad, h + 2 * pad);
    for y in 0..out.h {
        for x in 0..out.w {
            // backdrop in art pixels, so it is zoomed like the sprite
            let (ax, ay) = ((x - pad).div_euclid(z.num.max(1)) * z.den, (y - pad).div_euclid(z.num.max(1)) * z.den);
            let mut c = back(ax, ay);
            if x >= pad && y >= pad && x < pad + w && y < pad + h {
                let (sx, sy) = ((x - pad) * z.den / z.num, (y - pad) * z.den / z.num);
                c = over(src.get(sx, sy), c);
            }
            out.set(x, y, c);
        }
    }
    out
}

fn scene_back(s: &Scene) -> impl Fn(i32, i32) -> Rgba + '_ {
    move |x, y| s.at(x, y)
}

fn silhouette(src: &Canvas) -> Canvas {
    let mut out = src.clone();
    for p in &mut out.px {
        *p = if p[3] > 0 { [16, 16, 16, p[3]] } else { CLEAR };
    }
    out
}

/// Silhouette on light grey with the collision box (and disk/diamond) and the origin.
fn mask_panel(item: &Item, frame: &Canvas, z: Zoom) -> Canvas {
    let pad = 8;
    let mut out = panel(&silhouette(frame), z, pad, &|_, _| rgb(208, 208, 208));
    let (l, t, r, b) = item.bbox;
    let (x0, y0, x1, y1) = (pad + z.px(l), pad + z.px(t), pad + z.px(r + 1) - 1, pad + z.px(b + 1) - 1);
    out.frame_box(x0, y0, x1 - x0 + 1, y1 - y0 + 1, MASK);
    let (cx, cy, rx, ry) = ((x0 + x1) as f64 / 2.0 + 0.5, (y0 + y1) as f64 / 2.0 + 0.5, (x1 - x0 + 1) as f64 / 2.0, (y1 - y0 + 1) as f64 / 2.0);
    match item.mask {
        2 => {
            let mut ring = Canvas::new(out.w, out.h);
            ring.ellipse(cx, cy, rx, ry, MASK);
            for y in 0..out.h {
                for x in 0..out.w {
                    let edge = [(0, -1), (0, 1), (-1, 0), (1, 0)].iter().any(|(dx, dy)| !ring.opaque(x + dx, y + dy));
                    if ring.opaque(x, y) && edge {
                        out.set(x, y, MASK);
                    }
                }
            }
        }
        3 => {
            let (cx, cy) = (cx as i32, cy as i32);
            out.line(x0, cy, cx, y0, MASK);
            out.line(cx, y0, x1, cy, MASK);
            out.line(x1, cy, cx, y1, MASK);
            out.line(cx, y1, x0, cy, MASK);
        }
        _ => {}
    }
    let (ox, oy) = (pad + z.px(item.origin.0), pad + z.px(item.origin.1));
    for d in -4..=4 {
        out.set(ox + d, oy, ORIGIN);
        out.set(ox, oy + d, ORIGIN);
    }
    out
}

/// Frame `cur` against `prev`: red = only before, green = only now, yellow = changed
/// colour, grey = the same.
fn motion_panel(prev: &Canvas, cur: &Canvas, z: Zoom) -> Canvas {
    let mut m = Canvas::new(cur.w, cur.h);
    for y in 0..cur.h {
        for x in 0..cur.w {
            let (a, b) = (prev.get(x, y), cur.get(x, y));
            let c = match (a[3] > 0, b[3] > 0) {
                (true, false) => rgb(225, 50, 50),
                (false, true) => rgb(60, 200, 80),
                (true, true) if a != b => rgb(230, 200, 40),
                (true, true) => rgb(95, 95, 100),
                _ => CLEAR,
            };
            m.set(x, y, c);
        }
    }
    panel(&m, z, 4, &|_, _| DARK)
}

/// A row of panels with captions, wrapped to `width`.
struct Flow {
    width: i32,
    items: Vec<(Canvas, String)>,
}

impl Flow {
    fn new(width: i32) -> Self {
        Flow { width, items: Vec::new() }
    }
    fn add(&mut self, c: Canvas, caption: impl Into<String>) {
        self.items.push((c, caption.into()));
    }
    fn render(self) -> Canvas {
        let mut lines: Vec<Vec<(Canvas, String)>> = vec![Vec::new()];
        let mut x = 0;
        for (c, cap) in self.items {
            let w = c.w.max(text_width(&cap, 1));
            if x > 0 && x + w > self.width {
                lines.push(Vec::new());
                x = 0;
            }
            x += w + GAP;
            lines.last_mut().unwrap().push((c, cap));
        }
        let heights: Vec<i32> = lines.iter().map(|l| l.iter().map(|(c, _)| c.h).max().unwrap_or(0) + CHAR_H + 2).collect();
        let width = lines.iter().map(|l| l.iter().map(|(c, cap)| c.w.max(text_width(cap, 1)) + GAP).sum::<i32>() - GAP).max().unwrap_or(0).max(1);
        let height = heights.iter().sum::<i32>() + GAP * (lines.len() as i32 - 1);
        let mut out = Canvas::new(width, height.max(1));
        let mut y = 0;
        for (line, h) in lines.iter().zip(&heights) {
            let mut x = 0;
            for (c, cap) in line {
                out.paste(c, x, y);
                draw_text(&mut out, x, y + c.h + 2, cap, SUBTEXT, 1);
                x += c.w.max(text_width(cap, 1)) + GAP;
            }
            y += h + GAP;
        }
        out
    }
}

fn mask_name(m: i32) -> &'static str {
    match m {
        0 => "precise",
        2 => "disk",
        3 => "diamond",
        _ => "rect",
    }
}

fn sprite_card(item: &Item, scene: &Scene, o: &Options, inner: i32) -> Canvas {
    let (w, h) = item.size();
    let detail = o.detail;
    let z = pick_zoom(w, h, if detail { 160 } else { 96 }, if detail { 640 } else { 320 }, o.zoom);
    let back = scene_back(scene);
    let mut header = vec![format!("{}×{}", w, h)];
    header.push(if item.frames.len() == 1 { "1 frame".into() } else { format!("{} frames", item.frames.len()) });
    header.push(format!("origin {},{}", item.origin.0, item.origin.1));
    let (l, t, r, b) = item.bbox;
    header.push(format!("mask {} {},{}-{},{}", mask_name(item.mask), l, t, r, b));
    header.push(format!("zoom {}", z.label()));
    header.push(format!("on {}", scene.label));

    let mut flow = Flow::new(inner);
    for (i, f) in item.frames.iter().enumerate() {
        flow.add(panel(f, z, 0, &back), i.to_string());
    }
    let first = &item.frames[0];
    let mut flow2 = Flow::new(inner);
    flow2.add(panel(first, z, 4, &|_, _| LIGHT), "light");
    flow2.add(panel(first, z, 4, &|_, _| DARK), "dark");
    flow2.add(mask_panel(item, first, z), "mask+origin");
    // real size: every frame side by side at 1x (and 2x for small sprites)
    let real = |k: i32| {
        let n = item.frames.len() as i32;
        let cols = ((inner / k - 4) / (w + 4)).clamp(1, n);
        let rows = (n + cols - 1) / cols;
        let mut strip = Canvas::new((w + 4) * cols + 4, (h + 4) * rows + 4);
        for (i, f) in item.frames.iter().enumerate() {
            let i = i as i32;
            strip.paste(f, 4 + (i % cols) * (w + 4), 4 + (i / cols) * (h + 4));
        }
        panel(&strip, Zoom { num: k, den: 1 }, 0, &back)
    };
    if w * h <= 512 * 512 {
        flow2.add(real(1), "1×");
        if w.max(h) <= 48 {
            flow2.add(real(2), "2×");
        }
    }
    let mut rows = vec![flow.render(), flow2.render()];
    if detail && item.frames.len() > 1 {
        let mut fl = Flow::new(inner);
        let n = item.frames.len();
        for i in 0..n {
            let prev = &item.frames[(i + n - 1) % n];
            fl.add(motion_panel(prev, &item.frames[i], z), format!("{}>{}", (i + n - 1) % n, i));
        }
        rows.push(fl.render());
    }
    card(&item.name, &header.join("  "), rows)
}

fn background_card(item: &Item, o: &Options, inner: i32) -> Canvas {
    let img = &item.frames[0];
    let (w, h) = (img.w, img.h);
    let limit = if o.detail { inner } else { inner.min(560) };
    let z = pick_zoom(w, h, if o.detail { 512 } else { 192 }, limit, o.zoom);
    let mut header = vec![format!("{}×{}", w, h), format!("zoom {}", z.label())];
    let mut flow = Flow::new(inner);
    let mut shown = panel(img, z, 0, &checker);
    if let Some([tw, th, hs, vs, ho, vo]) = item.tileset {
        header.push(format!("tileset {tw}×{th}"));
        // tile boundaries
        let grid = rgb(255, 0, 200);
        let mut x = ho;
        while x < w && tw > 0 {
            for yy in (0..shown.h).step_by(2) {
                shown.set(z.px(x), yy, grid);
                shown.set(z.px(x + tw) - 1, yy, grid);
            }
            x += tw + hs;
        }
        let mut y = vo;
        while y < h && th > 0 {
            for xx in (0..shown.w).step_by(2) {
                shown.set(xx, z.px(y), grid);
                shown.set(xx, z.px(y + th) - 1, grid);
            }
            y += th + vs;
        }
        flow.add(shown, "tiles");
    } else {
        flow.add(shown, "image");
        // the wrap seams: a small image tiled 3x3, or the corner where four copies meet
        let tiles = item.wrap || item.tiled.0 || item.tiled.1;
        if !tiles {
        } else if w <= 96 && h <= 96 {
            let mut t = Canvas::new(w * 3, h * 3);
            for j in 0..3 {
                for i in 0..3 {
                    t.paste(img, i * w, j * h);
                }
            }
            let zz = pick_zoom(t.w, t.h, 288, 400, None);
            flow.add(panel(&t, zz, 0, &checker), "tiled 3×3");
        } else {
            let (cw, ch) = (w.min(96), h.min(96));
            let mut c = Canvas::new(cw, ch);
            for y in 0..ch {
                for x in 0..cw {
                    c.set(x, y, img.get((x - cw / 2).rem_euclid(w), (y - ch / 2).rem_euclid(h)));
                }
            }
            flow.add(panel(&c, Zoom { num: 3, den: 1 }, 0, &checker), "wrap corner (seams cross at the centre)");
        }
    }
    card(&item.name, &header.join("  "), vec![flow.render()])
}

fn checker(x: i32, y: i32) -> Rgba {
    if (x.div_euclid(8) + y.div_euclid(8)) % 2 == 0 {
        rgb(90, 90, 90)
    } else {
        rgb(120, 120, 120)
    }
}

fn card(title: &str, sub: &str, rows: Vec<Canvas>) -> Canvas {
    let content_w = rows.iter().map(|r| r.w).max().unwrap_or(0);
    let w = content_w.max(text_width(title, 2)).max(text_width(sub, 1)) + 2 * PAD;
    let h = PAD + 7 * 2 + 4 + CHAR_H + 6 + rows.iter().map(|r| r.h + GAP).sum::<i32>() + PAD - GAP;
    let mut out = Canvas::new(w, h);
    out.rect(0, 0, w, h, CARD_BG);
    draw_text(&mut out, PAD, PAD, title, TEXT, 2);
    draw_text(&mut out, PAD, PAD + 18, sub, SUBTEXT, 1);
    let mut y = PAD + 18 + CHAR_H + 6;
    for r in rows {
        out.paste(&r, PAD, y);
        y += r.h + GAP;
    }
    out
}

/// Renders the cards and packs them into pages.
pub fn render(items: &[(Item, Scene)], o: &Options) -> Vec<Canvas> {
    let inner = o.width - 2 * PAD - 2 * GAP;
    let cards: Vec<Canvas> = items
        .iter()
        .map(|(it, sc)| if it.is_sprite && !it.frames.is_empty() { sprite_card(it, sc, o, inner) } else { background_card(it, o, inner) })
        .collect();
    // shelf packing
    let mut pages: Vec<Vec<(i32, i32, usize)>> = vec![Vec::new()];
    let (mut x, mut y, mut shelf) = (GAP, GAP, 0);
    for (i, c) in cards.iter().enumerate() {
        if x > GAP && x + c.w > o.width - GAP {
            x = GAP;
            y += shelf + GAP;
            shelf = 0;
        }
        if y > GAP && y + c.h > o.page_height {
            pages.push(Vec::new());
            x = GAP;
            y = GAP;
            shelf = 0;
        }
        pages.last_mut().unwrap().push((x, y, i));
        x += c.w + GAP;
        shelf = shelf.max(c.h);
    }
    pages
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(|p| {
            let w = p.iter().map(|(x, _, i)| x + cards[*i].w).max().unwrap() + GAP;
            let h = p.iter().map(|(_, y, i)| y + cards[*i].h).max().unwrap() + GAP;
            let mut page = Canvas::new(w.max(o.width.min(w)), h);
            page.rect(0, 0, page.w, page.h, PAGE_BG);
            for (x, y, i) in p {
                page.paste(&cards[i], x, y);
            }
            page
        })
        .collect()
}

pub fn write_png(c: &Canvas, path: &std::path::Path) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), c.w as u32, c.h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    let data: Vec<u8> = c.px.iter().flatten().copied().collect();
    wr.write_image_data(&data).map_err(|e| e.to_string())
}
