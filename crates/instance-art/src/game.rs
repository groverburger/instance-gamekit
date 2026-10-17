//! Games: what `gmk-art` reads from a project (sprites, backgrounds, where they are
//! used) and how `build` writes `.art` resources into it.

use crate::art::{shown, ArtSet, Bbox, Kind, Resource};
use crate::canvas::Canvas;
use crate::color::*;
use instance_format::{Background, NodeStatus, Project, ResourceKind, Sprite, TreeNode};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// A sprite or background, from a game or from `.art` files.
#[derive(Clone, Debug)]
pub struct Item {
    pub name: String,
    pub is_sprite: bool,
    pub frames: Vec<Canvas>,
    pub origin: (i32, i32),
    /// Collision shape: 0 precise, 1 rectangle, 2 disk, 3 diamond.
    pub mask: i32,
    /// 0 automatic, 1 full image, 2 manual.
    pub bbox_mode: i32,
    pub bbox: (i32, i32, i32, i32),
    pub alpha_tolerance: i32,
    /// Background used as a tileset: width, height, h separation, v separation, h offset, v offset.
    pub tileset: Option<[i32; 6]>,
    /// From `against` in its `.art` source.
    pub against: Vec<String>,
    /// Declared seamless (`wrap`) in its `.art` source.
    pub wrap: bool,
    /// Tiled (horizontally, vertically) by a room background layer.
    pub tiled: (bool, bool),
}

impl Item {
    pub fn size(&self) -> (i32, i32) {
        self.frames.first().map(|f| (f.w, f.h)).unwrap_or((0, 0))
    }
}

pub fn sprite_item(s: &Sprite) -> Item {
    Item {
        name: s.name.clone(),
        is_sprite: true,
        frames: s.frames.iter().map(Canvas::from_image).collect(),
        origin: (s.origin_x, s.origin_y),
        mask: s.collision_shape,
        bbox_mode: s.bbox_mode,
        bbox: (s.bbox_left, s.bbox_top, s.bbox_right, s.bbox_bottom),
        alpha_tolerance: s.alpha_tolerance,
        tileset: None,
        against: Vec::new(),
        wrap: false,
        tiled: (false, false),
    }
}

pub fn background_item(b: &Background) -> Item {
    Item {
        name: b.name.clone(),
        is_sprite: false,
        frames: vec![Canvas::from_image(&b.image)],
        origin: (0, 0),
        mask: 1,
        bbox_mode: 1,
        bbox: (0, 0, b.image.width as i32 - 1, b.image.height as i32 - 1),
        alpha_tolerance: 0,
        tileset: b.use_as_tileset.then_some([b.tile_width, b.tile_height, b.tile_h_sep, b.tile_v_sep, b.tile_h_offset, b.tile_v_offset]),
        against: Vec::new(),
        wrap: false,
        tiled: (false, false),
    }
}

pub fn resource_item(r: &Resource) -> Item {
    let bbox = r.bbox_rect();
    Item {
        name: r.name.clone(),
        is_sprite: r.kind != Kind::Background,
        frames: r.frames.clone(),
        origin: r.origin,
        mask: r.mask,
        bbox_mode: match r.bbox {
            Bbox::Auto => 0,
            Bbox::Full => 1,
            Bbox::Manual(..) => 2,
        },
        bbox,
        alpha_tolerance: 0,
        tileset: r.tiles,
        against: r.against.clone(),
        wrap: r.wrap,
        tiled: (false, false),
    }
}

/// Every sprite then every background of a project; `against`/`wrap` hints come from
/// the matching `.art` resources, if any.
pub fn project_items(p: &Project, set: Option<&ArtSet>) -> Vec<Item> {
    let mut out: Vec<Item> = p.sprites.iter().flatten().map(sprite_item).collect();
    out.extend(p.backgrounds.iter().flatten().map(background_item));
    for b in p.rooms.iter().flatten().flat_map(|r| r.backgrounds.iter()) {
        if let Some(Some(bg)) = (b.visible && b.background >= 0).then(|| p.backgrounds.get(b.background as usize)).flatten() {
            if let Some(it) = out.iter_mut().find(|i| !i.is_sprite && i.name == bg.name) {
                it.tiled.0 |= b.tile_h;
                it.tiled.1 |= b.tile_v;
            }
        }
    }
    if let Some(set) = set {
        for it in &mut out {
            if let Some(r) = set.resources.iter().find(|r| r.name == it.name) {
                it.against = r.against.clone();
                it.wrap = r.wrap;
            }
        }
    }
    out
}

pub fn set_items(set: &ArtSet) -> Vec<Item> {
    let mut v: Vec<&std::rc::Rc<Resource>> = set.resources.iter().filter(|r| r.kind != Kind::Background).collect();
    v.extend(set.resources.iter().filter(|r| r.kind == Kind::Background));
    v.into_iter().map(|r| resource_item(r)).collect()
}

// ---------------------------------------------------------------------------------
// Usage: objects, rooms, rotation and flipping

/// How a sprite is used by the game's objects.
#[derive(Clone, Debug, Default)]
pub struct Usage {
    pub objects: BTreeSet<String>,
    /// Objects using it whose code sets `image_xscale` / `image_angle`.
    pub flipped_by: BTreeSet<String>,
    pub rotated_by: BTreeSet<String>,
    /// Rooms (indices) where one of those objects, or an object whose code names the
    /// sprite (`draw_sprite(spr_x, ...)`), is placed.
    pub rooms: BTreeSet<i32>,
}

/// Every identifier in `code`.
fn identifiers(code: &str) -> BTreeSet<&str> {
    code.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).filter(|t| !t.is_empty()).collect()
}

fn object_code(p: &Project, idx: usize) -> String {
    let mut code = String::new();
    let mut cur = idx as i32;
    let mut guard = 0;
    while cur >= 0 && (cur as usize) < p.objects.len() && guard < 64 {
        let Some(o) = &p.objects[cur as usize] else { break };
        for e in &o.events {
            for a in &e.actions {
                for s in &a.args {
                    code.push_str(s);
                    code.push('\n');
                }
            }
        }
        cur = o.parent;
        guard += 1;
    }
    code
}

fn ident_at(s: &str) -> &str {
    let end = s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(s.len());
    &s[..end]
}

/// Is `var` assigned (`=`, `*=`, `+=`, `-=`, `/=`; not `==`) somewhere in `code`?
pub fn assigns(code: &str, var: &str) -> bool {
    let mut from = 0;
    while let Some(i) = code[from..].find(var) {
        let at = from + i;
        from = at + var.len();
        let before_ok = code[..at].chars().next_back().is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
        let rest = code[from..].trim_start();
        let after_ok = (rest.starts_with('=') && !rest.starts_with("=="))
            || ["*=", "+=", "-=", "/=", ":="].iter().any(|op| rest.starts_with(op));
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// Sprite names assigned to `sprite_index` in `code`.
fn sprite_assignments(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = code[from..].find("sprite_index") {
        from += i + "sprite_index".len();
        let rest = code[from..].trim_start();
        if let Some(r) = rest.strip_prefix('=').or_else(|| rest.strip_prefix(":=")) {
            if !r.starts_with('=') {
                let id = ident_at(r.trim_start());
                if !id.is_empty() {
                    out.push(id.to_string());
                }
            }
        }
    }
    out
}

pub fn usage(p: &Project) -> HashMap<String, Usage> {
    let sprite_name = |i: i32| -> Option<String> { p.sprites.get(i as usize).and_then(|s| s.as_ref()).map(|s| s.name.clone()) };
    let mut map: HashMap<String, Usage> = HashMap::new();
    let mut placed: HashMap<i32, BTreeSet<i32>> = HashMap::new();
    for (ri, r) in p.rooms.iter().enumerate() {
        if let Some(r) = r {
            for inst in &r.instances {
                placed.entry(inst.object).or_default().insert(ri as i32);
            }
        }
    }
    for (oi, o) in p.objects.iter().enumerate() {
        let Some(o) = o else { continue };
        let code = object_code(p, oi);
        let mut names: BTreeSet<String> = sprite_assignments(&code).into_iter().collect();
        if let Some(n) = sprite_name(o.sprite) {
            names.insert(n);
        }
        let flips = assigns(&code, "image_xscale");
        let rotates = assigns(&code, "image_angle");
        for n in names {
            if !p.sprites.iter().flatten().any(|s| s.name == n) {
                continue;
            }
            let u = map.entry(n).or_default();
            u.objects.insert(o.name.clone());
            if flips {
                u.flipped_by.insert(o.name.clone());
            }
            if rotates {
                u.rotated_by.insert(o.name.clone());
            }
            if let Some(rs) = placed.get(&(oi as i32)) {
                u.rooms.extend(rs);
            }
        }
        // sprites drawn from this object's code are seen in its rooms
        if let Some(rs) = placed.get(&(oi as i32)) {
            for id in identifiers(&code) {
                if p.sprites.iter().flatten().any(|s| s.name == id) {
                    map.entry(id.to_string()).or_default().rooms.extend(rs);
                }
            }
        }
    }
    map
}

// ---------------------------------------------------------------------------------
// Scenes: what a sprite is seen against

/// A backdrop: a colour, an image drawn over it, or both.
#[derive(Clone, Debug)]
pub struct Scene {
    pub label: String,
    pub color: Option<Rgba>,
    pub image: Option<Canvas>,
}

impl Scene {
    pub fn plain(label: &str, c: Rgba) -> Scene {
        Scene { label: label.into(), color: Some(c), image: None }
    }

    /// The backdrop pixel at (x, y), sampled from the middle of the image.
    pub fn at(&self, x: i32, y: i32) -> Rgba {
        let base = self.color.unwrap_or(rgb(0, 0, 0));
        match &self.image {
            Some(img) if img.w > 0 && img.h > 0 => {
                let p = img.get((x + img.w / 2).rem_euclid(img.w), (y + img.h / 3).rem_euclid(img.h));
                over(p, base)
            }
            _ => base,
        }
    }

    /// Luminances of up to ~4000 backdrop pixels, sorted.
    pub fn luminances(&self) -> Vec<f64> {
        let mut out = Vec::new();
        match &self.image {
            Some(img) if img.w > 0 && img.h > 0 => {
                let step = ((img.w as f64 * img.h as f64 / 4000.0).sqrt().ceil() as i32).max(1);
                let base = self.color.unwrap_or(rgb(0, 0, 0));
                for y in (0..img.h).step_by(step as usize) {
                    for x in (0..img.w).step_by(step as usize) {
                        out.push(luminance(over(img.get(x, y), base)));
                    }
                }
            }
            _ => out.push(luminance(self.color.unwrap_or(rgb(0, 0, 0)))),
        }
        out.sort_by(|a, b| a.partial_cmp(b).unwrap());
        out
    }
}

/// A scene named by `spec`: `#hex`, a background name or a room name.
pub fn named_scene(p: Option<&Project>, spec: &str) -> Result<Scene, String> {
    if spec.starts_with('#') {
        return parse_hex(spec).map(|c| Scene::plain(spec, c)).ok_or(format!("`{spec}` is not a colour"));
    }
    let Some(p) = p else { return Err(format!("`{spec}`: background names need a game")) };
    if let Some(b) = p.backgrounds.iter().flatten().find(|b| b.name == spec) {
        return Ok(Scene { label: spec.into(), color: None, image: Some(Canvas::from_image(&b.image)) });
    }
    if let Some(ri) = p.rooms.iter().position(|r| r.as_ref().is_some_and(|r| r.name == spec)) {
        return Ok(room_scene(p, ri as i32));
    }
    Err(format!("`{spec}` is not a colour, background or room"))
}

/// The room as the player first sees it, without instances: its colour, background
/// layers and tiles, inside view 0 when views are on (at most 2048x2048).
pub fn room_scene(p: &Project, ri: i32) -> Scene {
    let r = p.rooms[ri as usize].as_ref().unwrap();
    let color = if r.draw_background_color { from_gm(r.background_color) } else { rgb(0, 0, 0) };
    let (mut x0, mut y0, mut w, mut h) = (0, 0, r.width, r.height);
    if r.views_enabled {
        if let Some(v) = r.views.iter().find(|v| v.visible) {
            (x0, y0, w, h) = (v.view_x, v.view_y, v.view_w, v.view_h);
        }
    }
    let (w, h) = (w.clamp(1, 2048), h.clamp(1, 2048));
    let mut cv = Canvas::new(w, h);
    cv.rect(0, 0, w, h, color);
    let bg = |i: i32| p.backgrounds.get(i as usize).and_then(|b| b.as_ref());
    let mut parts = Vec::new();
    for l in r.backgrounds.iter().filter(|l| l.visible && !l.is_foreground) {
        let Some(b) = bg(l.background) else { continue };
        let img = Canvas::from_image(&b.image);
        if img.w == 0 || img.h == 0 {
            continue;
        }
        parts.push(b.name.clone());
        for y in 0..h {
            for x in 0..w {
                let (rx, ry) = (x + x0, y + y0);
                let (mut ix, mut iy) = if l.stretch {
                    (rx * img.w / r.width.max(1), ry * img.h / r.height.max(1))
                } else {
                    (rx - l.x, ry - l.y)
                };
                if l.tile_h {
                    ix = ix.rem_euclid(img.w);
                }
                if l.tile_v {
                    iy = iy.rem_euclid(img.h);
                }
                let c = img.get(ix, iy);
                if c[3] > 0 {
                    cv.blend(x, y, c);
                }
            }
        }
    }
    let mut tiles: Vec<_> = r.tiles.iter().collect();
    tiles.sort_by_key(|t| std::cmp::Reverse(t.depth));
    let mut images: HashMap<i32, Canvas> = HashMap::new();
    for t in &tiles {
        let Some(b) = bg(t.background) else { continue };
        let img = images.entry(t.background).or_insert_with(|| Canvas::from_image(&b.image));
        for y in 0..t.height {
            for x in 0..t.width {
                let c = img.get(t.tile_x + x, t.tile_y + y);
                if c[3] > 0 {
                    cv.blend(t.x - x0 + x, t.y - y0 + y, c);
                }
            }
        }
    }
    if !tiles.is_empty() {
        parts.push("tiles".into());
    }
    let label = if parts.is_empty() { r.name.clone() } else { format!("{} ({})", r.name, parts.join(", ")) };
    Scene { label, color: Some(color), image: Some(cv) }
}

/// The scene a sprite is judged against: `--bg`, then its `against`, then the first
/// room (in room order) where it is placed, then the largest room.
pub fn scene_for(p: Option<&Project>, item: &Item, usage: Option<&Usage>, forced: Option<&Scene>) -> Option<Scene> {
    if let Some(s) = forced {
        return Some(s.clone());
    }
    if let Some(a) = item.against.first() {
        if let Ok(s) = named_scene(p, a) {
            return Some(s);
        }
    }
    let p = p?;
    if p.room_order.is_empty() {
        return None;
    }
    if let Some(u) = usage {
        if let Some(ri) = p.room_order.iter().find(|ri| u.rooms.contains(ri)) {
            return Some(room_scene(p, *ri));
        }
    }
    let largest = p
        .room_order
        .iter()
        .filter_map(|ri| p.rooms.get(*ri as usize).and_then(|r| r.as_ref()).map(|r| (*ri, r.width as i64 * r.height as i64)))
        .max_by_key(|(ri, area)| (*area, -ri))?;
    Some(room_scene(p, largest.0))
}

// ---------------------------------------------------------------------------------
// Building `.art` resources into a project

#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    Added,
    Updated,
    Unchanged,
}

fn root_mut(p: &mut Project, kind: ResourceKind) -> Option<&mut TreeNode> {
    p.resource_tree.iter_mut().find(|n| n.kind == kind && n.status == NodeStatus::Root)
}

fn sprite_from(r: &Resource, old: Option<&Sprite>) -> Sprite {
    let (l, t, rr, b) = r.bbox_rect();
    let mut s = old.cloned().unwrap_or_default();
    s.name = r.name.clone();
    s.frames = r.frames.iter().map(Canvas::image).collect();
    (s.origin_x, s.origin_y) = r.origin;
    s.collision_shape = r.mask;
    s.bbox_mode = match r.bbox {
        Bbox::Auto => 0,
        Bbox::Full => 1,
        Bbox::Manual(..) => 2,
    };
    (s.bbox_left, s.bbox_top, s.bbox_right, s.bbox_bottom) = (l, t, rr, b);
    if s.bbox_mode == 0 {
        s.alpha_tolerance = 0;
    }
    s
}

fn background_from(r: &Resource, old: Option<&Background>) -> Background {
    let mut b = old.cloned().unwrap_or_default();
    b.name = r.name.clone();
    b.image = r.frames[0].image();
    match r.tiles {
        Some([w, h, hs, vs, ho, vo]) => {
            b.use_as_tileset = true;
            (b.tile_width, b.tile_height, b.tile_h_sep, b.tile_v_sep, b.tile_h_offset, b.tile_v_offset) = (w, h, hs, vs, ho, vo);
        }
        None => b.use_as_tileset = false,
    }
    b
}

/// Writes every sprite and background of `set` into `p` (by name; new ones get the
/// next slot and a leaf at the end of their resource tree root).
pub fn apply(p: &mut Project, set: &ArtSet, only: &[String]) -> Result<Vec<(String, Change)>, String> {
    let mut out = Vec::new();
    for r in &set.resources {
        if r.kind == Kind::Piece || (!only.is_empty() && !only.contains(&r.name)) {
            continue;
        }
        let at = format!("{}:{}", shown(&r.file), r.line);
        if r.kind == Kind::Sprite {
            if p.backgrounds.iter().flatten().any(|b| b.name == r.name) {
                return Err(format!("{at}: `{}` is a background in the game, not a sprite", r.name));
            }
            match p.sprites.iter_mut().flatten().find(|s| s.name == r.name) {
                Some(s) => {
                    let new = sprite_from(r, Some(s));
                    let changed = *s != new;
                    *s = new;
                    out.push((r.name.clone(), if changed { Change::Updated } else { Change::Unchanged }));
                }
                None => {
                    p.sprites.push(Some(sprite_from(r, None)));
                    let idx = p.sprites.len() as i32 - 1;
                    if let Some(root) = root_mut(p, ResourceKind::Sprite) {
                        root.children.push(TreeNode::leaf(ResourceKind::Sprite, idx, r.name.clone()));
                    }
                    out.push((r.name.clone(), Change::Added));
                }
            }
        } else {
            if p.sprites.iter().flatten().any(|s| s.name == r.name) {
                return Err(format!("{at}: `{}` is a sprite in the game, not a background", r.name));
            }
            if r.frames.len() > 1 {
                return Err(format!("{at}: a background has one image, `{}` has {} frames", r.name, r.frames.len()));
            }
            match p.backgrounds.iter_mut().flatten().find(|b| b.name == r.name) {
                Some(b) => {
                    let new = background_from(r, Some(b));
                    let changed = *b != new;
                    *b = new;
                    out.push((r.name.clone(), if changed { Change::Updated } else { Change::Unchanged }));
                }
                None => {
                    p.backgrounds.push(Some(background_from(r, None)));
                    let idx = p.backgrounds.len() as i32 - 1;
                    if let Some(root) = root_mut(p, ResourceKind::Background) {
                        root.children.push(TreeNode::leaf(ResourceKind::Background, idx, r.name.clone()));
                    }
                    out.push((r.name.clone(), Change::Added));
                }
            }
        }
    }
    let icons: Vec<&std::rc::Rc<Resource>> = set.resources.iter().filter(|r| r.icon).collect();
    if icons.len() > 1 {
        return Err(format!("`icon` is set on both `{}` and `{}`", icons[0].name, icons[1].name));
    }
    if let Some(r) = icons.first() {
        if only.is_empty() || only.contains(&r.name) {
            let ico = make_ico(&r.frames[0])?;
            let changed = p.settings.icon != ico;
            p.settings.icon = ico;
            out.push(("(game icon)".into(), if changed { Change::Updated } else { Change::Unchanged }));
        }
    }
    for o in only {
        if !set.resources.iter().any(|r| &r.name == o) {
            return Err(format!("`{o}` is not defined in the .art files"));
        }
    }
    Ok(out)
}

/// A Windows icon (one 32-bit image, up to 256x256) made from a canvas.
pub fn make_ico(src: &Canvas) -> Result<Vec<u8>, String> {
    let (w, h) = (src.w as u32, src.h as u32);
    if w == 0 || h == 0 || w > 256 || h > 256 {
        return Err(format!("an icon is at most 256x256, not {w}x{h}"));
    }
    let xor_len = w * h * 4;
    let and_len = w.div_ceil(32) * 4 * h;
    let img_len = 40 + xor_len + and_len;
    let mut v = vec![0, 0, 1, 0, 1, 0];
    v.extend_from_slice(&[(w % 256) as u8, (h % 256) as u8, 0, 0, 1, 0, 32, 0]);
    v.extend_from_slice(&img_len.to_le_bytes());
    v.extend_from_slice(&22u32.to_le_bytes());
    v.extend_from_slice(&40u32.to_le_bytes());
    v.extend_from_slice(&(w as i32).to_le_bytes());
    v.extend_from_slice(&((h * 2) as i32).to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&32u16.to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&(xor_len + and_len).to_le_bytes());
    v.extend_from_slice(&[0; 16]);
    for row in 0..h as i32 {
        for x in 0..w as i32 {
            let c = src.get(x, h as i32 - 1 - row);
            v.extend_from_slice(&[c[2], c[1], c[0], c[3]]);
        }
    }
    v.extend(std::iter::repeat_n(0u8, and_len as usize));
    Ok(v)
}

/// What differs between an `.art` resource and the game's copy (empty when in sync).
pub fn out_of_date(p: &Project, r: &Resource) -> Option<String> {
    match r.kind {
        Kind::Piece => None,
        Kind::Sprite => match p.sprites.iter().flatten().find(|s| s.name == r.name) {
            None => Some("not in the game yet".into()),
            Some(s) => {
                let new = sprite_from(r, Some(s));
                if new.frames != s.frames {
                    Some("its frames differ from the .art source".into())
                } else if new != *s {
                    Some("its origin, mask or bbox differ from the .art source".into())
                } else {
                    None
                }
            }
        },
        Kind::Background => match p.backgrounds.iter().flatten().find(|b| b.name == r.name) {
            None => Some("not in the game yet".into()),
            Some(b) => (background_from(r, Some(b)) != *b).then(|| "it differs from the .art source".into()),
        },
    }
}

/// The names of the sprites and backgrounds in a project folder, read from their files
/// (without parsing the project, which fails while code refers to a missing resource).
fn folder_names(dir: &Path) -> (BTreeSet<String>, BTreeSet<String>) {
    let name_in = |toml: &Path| -> Option<String> {
        let text = std::fs::read_to_string(toml).ok()?;
        let line = text.lines().find(|l| l.trim_start().starts_with("name"))?;
        Some(line.split_once('=')?.1.trim().trim_matches('"').to_string())
    };
    let mut sprites = BTreeSet::new();
    if let Ok(rd) = std::fs::read_dir(dir.join("sprites")) {
        for e in rd.flatten().filter(|e| e.path().is_dir()) {
            sprites.insert(name_in(&e.path().join("sprite.toml")).unwrap_or_else(|| e.file_name().to_string_lossy().into_owned()));
        }
    }
    let mut backgrounds = BTreeSet::new();
    if let Ok(rd) = std::fs::read_dir(dir.join("backgrounds")) {
        for e in rd.flatten() {
            let p = e.path();
            match p.extension().and_then(|x| x.to_str()) {
                Some("toml") => {
                    backgrounds.insert(name_in(&p).unwrap_or_else(|| p.file_stem().unwrap().to_string_lossy().into_owned()));
                }
                Some("png") if !p.with_extension("toml").exists() => {
                    backgrounds.insert(p.file_stem().unwrap().to_string_lossy().into_owned());
                }
                _ => {}
            }
        }
    }
    (sprites, backgrounds)
}

/// Writes a first image for every `.art` sprite and background that the folder does not
/// have yet, so the folder can be read even when objects or rooms already refer to
/// them; `apply` then fills in the rest. Returns the names it added.
pub fn add_missing(dir: &Path, set: &ArtSet) -> Result<Vec<String>, String> {
    let (sprites, backgrounds) = folder_names(dir);
    let mut added = Vec::new();
    for r in &set.resources {
        let path = match r.kind {
            Kind::Sprite if !sprites.contains(&r.name) => dir.join("sprites").join(&r.name).join("0.png"),
            Kind::Background if !backgrounds.contains(&r.name) => dir.join("backgrounds").join(format!("{}.png", r.name)),
            _ => continue,
        };
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| format!("{}: {e}", shown(&path)))?;
        crate::sheet::write_png(&r.frames[0], &path)?;
        added.push(r.name.clone());
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_assignments() {
        assert!(assigns("image_xscale = -1;", "image_xscale"));
        assert!(assigns("x+=1; image_angle += 5", "image_angle"));
        assert!(!assigns("if image_xscale == 1 {}", "image_xscale"));
        assert!(!assigns("my_image_xscale = 2", "image_xscale"));
        assert_eq!(sprite_assignments("sprite_index = spr_run; if sprite_index == spr_a {}"), vec!["spr_run"]);
    }
}
