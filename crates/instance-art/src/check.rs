//! `gmk-art check`: measurements that turn art judgement calls into numbers.

use crate::art::{ArtSet, Kind};
use crate::canvas::Canvas;
use crate::color::*;
use crate::game::{out_of_date, Item, Scene, Usage};
use instance_format::Project;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Warning,
    Note,
}

#[derive(Serialize, Clone, Debug)]
pub struct Finding {
    pub name: String,
    pub level: Level,
    pub code: &'static str,
    pub message: String,
}

/// Measurements of one sprite or background.
#[derive(Serialize, Clone, Debug, Default)]
pub struct Stats {
    pub name: String,
    pub kind: &'static str,
    pub width: i32,
    pub height: i32,
    pub frames: usize,
    /// Distinct opaque colours.
    pub colours: usize,
    /// Each art pixel is drawn as an NxN block.
    pub pixel_scale: Option<i32>,
    /// Hard edges: (almost) no semi-transparent pixels.
    pub hard_edged: bool,
    /// The dark outline colour, if the edge is mostly one dark colour.
    pub outline: Option<String>,
    /// Backdrop it was judged against.
    pub scene: Option<String>,
    /// Share of edge pixels (0..1) that blend into the backdrop.
    pub blend: Option<f64>,
    /// Median contrast ratio of the edge against the backdrop.
    pub edge_contrast: Option<f64>,
    /// Wrap seam strength (seam difference / typical neighbour difference), backgrounds.
    pub seam: Option<(f64, f64)>,
}

#[derive(Serialize, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub stats: Vec<Stats>,
}

pub struct Input<'a> {
    pub items: &'a [Item],
    /// One per item.
    pub scenes: &'a [Option<Scene>],
    pub project: Option<&'a Project>,
    pub usage: Option<&'a HashMap<String, Usage>>,
    pub set: Option<&'a ArtSet>,
}

const SOLID: u8 = 128;
const N4: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

fn solid(c: &Canvas, x: i32, y: i32) -> bool {
    c.get(x, y)[3] >= SOLID
}

/// An edge pixel and the solid pixel one step inwards, if any.
type Edge = ((i32, i32), Option<(i32, i32)>);

/// Solid pixels with a non-solid 4-neighbour.
fn edges(c: &Canvas) -> Vec<Edge> {
    let mut out = Vec::new();
    for y in 0..c.h {
        for x in 0..c.w {
            if !solid(c, x, y) {
                continue;
            }
            if let Some((dx, dy)) = N4.iter().find(|(dx, dy)| !solid(c, x + dx, y + dy)) {
                let (ix, iy) = (x - dx, y - dy);
                out.push(((x, y), solid(c, ix, iy).then_some((ix, iy))));
            }
        }
    }
    out
}

fn is_edge(c: &Canvas, x: i32, y: i32) -> bool {
    solid(c, x, y) && N4.iter().any(|(dx, dy)| !solid(c, x + dx, y + dy))
}

/// (outline colour, share of the edge it covers, share of it that is 2 px thick).
fn outline_info(frames: &[Canvas]) -> Option<(Rgba, f64, f64)> {
    let mut count: HashMap<Rgba, usize> = HashMap::new();
    let mut total = 0;
    for f in frames {
        for ((x, y), _) in edges(f) {
            *count.entry(f.get(x, y)).or_default() += 1;
            total += 1;
        }
    }
    let (col, n) = count.into_iter().max_by_key(|(c, n)| (*n, *c))?;
    let share = n as f64 / total as f64;
    if total < 12 || share < 0.6 || luminance(col) > 0.08 {
        return None;
    }
    let (mut on, mut thick) = (0, 0);
    for f in frames {
        for ((x, y), inner) in edges(f) {
            if f.get(x, y) != col {
                continue;
            }
            on += 1;
            if let Some((ix, iy)) = inner {
                if f.get(ix, iy) == col && !is_edge(f, ix, iy) {
                    thick += 1;
                }
            }
        }
    }
    Some((col, share, thick as f64 / on.max(1) as f64))
}

/// The largest N (up to 8) such that the image is made of NxN blocks of one colour,
/// with the alignment of the blocks; 1 for ordinary pixel art.
fn pixel_scale(c: &Canvas) -> (i32, i32, i32) {
    if c.w * c.h > 256 * 256 {
        return (1, 0, 0);
    }
    for k in (2..=8).rev() {
        if c.w < 2 * k || c.h < 2 * k {
            continue;
        }
        for oy in 0..k {
            'align: for ox in 0..k {
                let mut by = oy - k;
                while by < c.h {
                    let mut bx = ox - k;
                    while bx < c.w {
                        let mut first: Option<Rgba> = None;
                        for y in by.max(0)..(by + k).min(c.h) {
                            for x in bx.max(0)..(bx + k).min(c.w) {
                                let p = c.get(x, y);
                                match first {
                                    None => first = Some(p),
                                    Some(f) if f != p => continue 'align,
                                    _ => {}
                                }
                            }
                        }
                        bx += k;
                    }
                    by += k;
                }
                return (k, ox, oy);
            }
        }
    }
    (1, 0, 0)
}

/// One pixel per NxN block: the image in art pixels.
fn downsample(c: &Canvas, (k, ox, oy): (i32, i32, i32)) -> Canvas {
    if k == 1 {
        return c.clone();
    }
    // block columns start at ox - k, ox, ox + k, ... (the first may be cut by the edge)
    let (bx0, by0) = (if ox == 0 { 0 } else { ox - k }, if oy == 0 { 0 } else { oy - k });
    let (w, h) = ((c.w - bx0 + k - 1) / k, (c.h - by0 + k - 1) / k);
    let mut out = Canvas::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = ((bx0 + x * k).max(0), (by0 + y * k).max(0));
            out.set(x, y, c.get(sx, sy));
        }
    }
    out
}

/// Mean colour distance across the wrap seam and between neighbouring columns (or
/// rows when `vertical`).
fn seam(c: &Canvas, vertical: bool) -> (f64, f64) {
    let (len, across) = if vertical { (c.w, c.h) } else { (c.h, c.w) };
    let px = |i: i32, j: i32| if vertical { c.get(i, j) } else { c.get(j, i) };
    let mut s = 0.0;
    for i in 0..len {
        s += distance(px(i, across - 1), px(i, 0));
    }
    let mut inner = 0.0;
    let mut n = 0;
    for j in 0..across - 1 {
        for i in (0..len).step_by(((len / 64).max(1)) as usize) {
            inner += distance(px(i, j), px(i, j + 1));
            n += 1;
        }
    }
    (s / len.max(1) as f64, inner / n.max(1) as f64)
}

pub fn run(input: &Input) -> Report {
    let mut rep = Report::default();
    let mut add = |name: &str, level: Level, code: &'static str, message: String| {
        rep.findings.push(Finding { name: name.to_string(), level, code, message });
    };

    let palette = input.set.and_then(|s| s.palette.clone());

    let mut stats: Vec<Stats> = Vec::new();
    for (idx, it) in input.items.iter().enumerate() {
        let (w, h) = it.size();
        let mut st = Stats {
            name: it.name.clone(),
            kind: if it.is_sprite { "sprite" } else { "background" },
            width: w,
            height: h,
            frames: it.frames.len(),
            ..Stats::default()
        };
        if it.frames.is_empty() {
            stats.push(st);
            continue;
        }
        let name = it.name.as_str();
        let mut colours: BTreeMap<Rgba, usize> = BTreeMap::new();
        let (mut partial, mut nonzero) = (0usize, 0usize);
        for f in &it.frames {
            for p in &f.px {
                if p[3] == 255 {
                    *colours.entry(*p).or_default() += 1;
                }
                if p[3] > 0 {
                    nonzero += 1;
                    if p[3] < 255 {
                        partial += 1;
                    }
                }
            }
        }
        st.colours = colours.len();
        st.hard_edged = partial * 10 <= nonzero;

        // -- strict palette
        if let Some(pal) = &palette {
            let mut off: Vec<(Rgba, usize)> = colours.iter().filter(|(c, _)| !pal.contains(c)).map(|(c, n)| (*c, *n)).collect();
            if !off.is_empty() {
                off.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
                let list: Vec<String> = off
                    .iter()
                    .take(5)
                    .map(|(c, n)| {
                        let near = pal.iter().min_by(|a, b| distance(**a, *c).partial_cmp(&distance(**b, *c)).unwrap()).unwrap();
                        format!("{} ({n} px, nearest {})", hex(*c), hex(*near))
                    })
                    .collect();
                add(
                    name,
                    Level::Warning,
                    "off-palette",
                    format!("{} colour(s) not in the strict palette: {}{}", off.len(), list.join(", "), if off.len() > 5 { ", ..." } else { "" }),
                );
            }
        }

        if !it.is_sprite {
            // -- backgrounds: tileset layout, wrap seams
            if let Some([tw, th, hs, vs, ho, vo]) = it.tileset {
                let fits = |size: i32, t: i32, sep: i32, off: i32| t > 0 && (size - off + sep) % (t + sep) == 0;
                if !fits(w, tw, hs, ho) || !fits(h, th, vs, vo) {
                    add(name, Level::Note, "tileset-size", format!("{w}x{h} is not a whole number of {tw}x{th} tiles (with separation {hs},{vs} and offset {ho},{vo})"));
                }
            } else {
                let (sh, ih) = seam(&it.frames[0], false);
                let (sv, iv) = seam(&it.frames[0], true);
                let (rh, rv) = (sh / (ih + 0.004), sv / (iv + 0.004));
                st.seam = Some((rh, rv));
                let (th, tv) = it.tiled;
                for (dir, s, r, tiled) in [("left/right", sh, rh, th), ("top/bottom", sv, rv, tv)] {
                    let why = if it.wrap {
                        "declared `wrap`"
                    } else if tiled {
                        "tiled in a room"
                    } else {
                        continue;
                    };
                    if s > 0.03 && r > 2.5 {
                        add(name, Level::Warning, "seam", format!("visible {dir} wrap seam ({why}): the edges differ {r:.1}x more than neighbouring pixels do"));
                    }
                }
            }
            stats.push(st);
            continue;
        }

        // -- frames and collision box
        if it.frames.iter().any(|f| f.w != w || f.h != h) {
            add(name, Level::Warning, "frame-size", "frames have different sizes".into());
        }
        let auto = it.frames.iter().filter_map(|f| f.opaque_box(it.alpha_tolerance.clamp(0, 255) as u8)).fold(None, |acc: Option<(i32, i32, i32, i32)>, (l, t, r, b)| {
            Some(match acc {
                None => (l, t, r, b),
                Some((l0, t0, r0, b0)) => (l0.min(l), t0.min(t), r0.max(r), b0.max(b)),
            })
        });
        if let (0, Some(e)) = (it.bbox_mode, auto) {
            if e != it.bbox {
                add(
                    name,
                    Level::Note,
                    "bbox-stale",
                    format!(
                        "the stored automatic bbox {},{}-{},{} differs from the image's {},{}-{},{} (the runner recomputes it; the IDE and sprite.toml show the old one)",
                        it.bbox.0, it.bbox.1, it.bbox.2, it.bbox.3, e.0, e.1, e.2, e.3
                    ),
                );
            }
        }
        let (ox, oy) = it.origin;
        if ox < 0 || oy < 0 || ox > w || oy > h {
            add(name, Level::Note, "origin-outside", format!("origin {ox},{oy} is outside the {w}x{h} image"));
        }
        if let Some(u) = input.usage.and_then(|m| m.get(name)) {
            let (l, t, r, b) = it.bbox;
            let (cx, cy) = ((l + r + 1) as f64 / 2.0, (t + b + 1) as f64 / 2.0);
            if !u.flipped_by.is_empty() && (ox as f64 - cx).abs() > 1.0 {
                add(
                    name,
                    Level::Warning,
                    "flip-origin",
                    format!(
                        "{} set image_xscale, but origin x {ox} is {:.1} px off the centre of the bbox ({cx}): the sprite jumps sideways when it flips",
                        list(&u.flipped_by),
                        (ox as f64 - cx).abs()
                    ),
                );
            }
            let off = ((ox as f64 - cx).powi(2) + (oy as f64 - cy).powi(2)).sqrt();
            if !u.rotated_by.is_empty() && off > 2.0_f64.max(0.15 * (w.max(h) as f64)) {
                add(
                    name,
                    Level::Warning,
                    "rotate-origin",
                    format!("{} set image_angle, but the origin {ox},{oy} is {off:.1} px from the centre of the bbox ({cx},{cy}): it swings around when it rotates", list(&u.rotated_by)),
                );
            }
        }

        // -- pixel faults
        let mut strays = Vec::new();
        let mut solid_count = 0;
        for (fi, f) in it.frames.iter().enumerate() {
            for y in 0..f.h {
                for x in 0..f.w {
                    if !solid(f, x, y) {
                        continue;
                    }
                    solid_count += 1;
                    let alone = (-1..=1).all(|dy| (-1..=1).all(|dx| (dx == 0 && dy == 0) || !solid(f, x + dx, y + dy)));
                    if alone {
                        strays.push((fi, x, y));
                    }
                }
            }
        }
        if !strays.is_empty() && strays.len() <= 6 && solid_count >= 30 {
            let at: Vec<String> =
                strays.iter().take(4).map(|(f, x, y)| if it.frames.len() > 1 { format!("{x},{y} (frame {f})") } else { format!("{x},{y}") }).collect();
            add(name, Level::Note, "stray-pixel", format!("{} isolated pixel(s) at {}", strays.len(), at.join(", ")));
        }
        if partial > 0 && st.hard_edged && solid_count >= 30 {
            add(name, Level::Note, "soft-edge", format!("{partial} semi-transparent pixel(s) on an otherwise hard-edged sprite (anti-aliased fringe?)"));
        }
        if st.colours <= 16 {
            let cs: Vec<&Rgba> = colours.keys().collect();
            let mut pairs = Vec::new();
            for i in 0..cs.len() {
                for j in i + 1..cs.len() {
                    let d = distance(*cs[i], *cs[j]);
                    if d < 0.012 {
                        pairs.push(format!("{} / {}", hex(*cs[i]), hex(*cs[j])));
                    }
                }
            }
            if !pairs.is_empty() {
                add(name, Level::Note, "near-duplicate", format!("colours that look the same: {}", pairs.iter().take(3).cloned().collect::<Vec<_>>().join(", ")));
            }
        }
        let scale = if st.hard_edged && w.min(h) >= 8 && st.colours >= 4 { pixel_scale(&it.frames[0]) } else { (1, 0, 0) };
        let art: Vec<Canvas> = it.frames.iter().map(|f| downsample(f, scale)).collect();
        let outline = if st.hard_edged { outline_info(&art) } else { None };
        if let Some((col, _, thick)) = outline {
            st.outline = Some(hex(col));
            if thick > 0.25 {
                add(name, Level::Note, "thick-outline", format!("the {} outline is 2 px thick along {:.0}% of the edge", hex(col), thick * 100.0));
            }
        }
        st.pixel_scale = (st.hard_edged && w.min(h) >= 8 && st.colours >= 4).then_some(scale.0);

        // -- contrast against the backdrop
        if let Some(scene) = &input.scenes[idx] {
            let lum = scene.luminances();
            let lb = lum[lum.len() / 2];
            let mut ratios = Vec::new();
            for f in &art {
                for ((x, y), inner) in edges(f) {
                    let a = contrast(luminance(f.get(x, y)), lb);
                    let b = inner.map(|(ix, iy)| contrast(luminance(f.get(ix, iy)), lb)).unwrap_or(1.0);
                    ratios.push(a.max(b));
                }
            }
            if ratios.len() >= 8 {
                ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let blend = ratios.iter().filter(|r| **r < 1.5).count() as f64 / ratios.len() as f64;
                st.scene = Some(scene.label.clone());
                st.blend = Some(blend);
                st.edge_contrast = Some(ratios[ratios.len() / 2]);
                if blend >= 0.5 {
                    add(
                        name,
                        if blend >= 0.75 { Level::Warning } else { Level::Note },
                        "contrast",
                        format!(
                            "{:.0}% of its edge blends into {} (median edge contrast {:.2}:1; aim for 2:1 or more): add an outline or a lighter/darker rim",
                            blend * 100.0,
                            scene.label,
                            ratios[ratios.len() / 2]
                        ),
                    );
                }
            }
        }
        stats.push(st);
    }

    // -- consistency across sprites
    let sprites: Vec<&Stats> = stats.iter().filter(|s| s.kind == "sprite" && s.hard_edged && s.width.min(s.height) >= 8).collect();
    if sprites.len() >= 4 {
        let with = sprites.iter().filter(|s| s.outline.is_some()).count();
        if with as f64 >= 0.7 * sprites.len() as f64 && with < sprites.len() {
            let without: Vec<&str> = sprites.iter().filter(|s| s.outline.is_none()).map(|s| s.name.as_str()).collect();
            add("*", Level::Note, "no-outline", format!("{with} of {} sprites have a dark outline; these do not: {}", sprites.len(), names(&without)));
        }
        let mut scales: BTreeMap<i32, usize> = BTreeMap::new();
        for s in &sprites {
            if let Some(k) = s.pixel_scale {
                *scales.entry(k).or_default() += 1;
            }
        }
        if let Some((&common, &n)) = scales.iter().max_by_key(|(_, n)| **n) {
            if n as f64 >= 0.6 * sprites.len() as f64 {
                let odd: Vec<String> =
                    sprites.iter().filter_map(|s| s.pixel_scale.filter(|k| *k != common).map(|k| format!("{} ({k}x{k})", s.name))).collect();
                if !odd.is_empty() {
                    let odd: Vec<&str> = odd.iter().map(String::as_str).collect();
                    add("*", Level::Note, "pixel-scale", format!("{n} of {} sprites are drawn with {common}x{common} pixels; these are not: {}", sprites.len(), names(&odd)));
                }
            }
        }
    }

    // -- .art sources that were not built
    if let (Some(p), Some(set)) = (input.project, input.set) {
        for r in set.resources.iter().filter(|r| r.kind != Kind::Piece) {
            if let Some(why) = out_of_date(p, r) {
                add(&r.name, Level::Warning, "not-built", format!("{why}: run `gmk-art build`"));
            }
        }
    }

    rep.findings.sort_by(|a, b| (a.level, &a.name).cmp(&(b.level, &b.name)));
    rep.stats = stats;
    rep
}

fn names(v: &[&str]) -> String {
    if v.len() <= 8 {
        v.join(", ")
    } else {
        format!("{} and {} more", v[..8].join(", "), v.len() - 8)
    }
}

fn list(s: &std::collections::BTreeSet<String>) -> String {
    let v: Vec<&str> = s.iter().map(String::as_str).take(3).collect();
    let more = if s.len() > 3 { format!(" and {} more", s.len() - 3) } else { String::new() };
    format!("{}{more}", v.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_scale_finds_blocks() {
        let mut c = Canvas::new(16, 16);
        c.rect(2, 2, 4, 4, rgb(255, 0, 0));
        c.rect(6, 2, 2, 2, rgb(0, 255, 0));
        assert_eq!(pixel_scale(&c).0, 2);
        c.set(9, 9, rgb(1, 1, 1));
        assert_eq!(pixel_scale(&c).0, 1);
    }

    #[test]
    fn outline_detection() {
        let mut c = Canvas::new(16, 16);
        c.ellipse(8.0, 8.0, 5.0, 5.0, rgb(200, 100, 50));
        c.outline(rgb(10, 10, 10), false, false);
        let (col, share, thick) = outline_info(&[c.clone()]).unwrap();
        assert_eq!(col, rgb(10, 10, 10));
        assert!(share > 0.95 && thick < 0.05);
        c.outline(rgb(10, 10, 10), false, false);
        let (_, _, thick) = outline_info(&[c]).unwrap();
        assert!(thick > 0.5, "{thick}");
    }
}
