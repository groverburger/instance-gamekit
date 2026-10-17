//! `gmk-art` — pixel art as text, contact sheets and art checks.
//!
//! ```text
//! gmk-art build GAME [--only NAMES] [--dry-run]
//! gmk-art sheet GAME|FILE.art ... [-o sheet.png] [--only NAMES] [--detail] [--zoom N]
//!               [--bg SCENE] [--width PX]
//! gmk-art check GAME|FILE.art ... [--only NAMES] [--bg SCENE] [--verbose] [--json]
//! ```

use instance_art::art::{art_files, load_set, shown, ArtSet, Kind};
use instance_art::check::{self, Level};
use instance_art::game::{self, Change, Item, Scene};
use instance_art::sheet;
use instance_format::{read_folder, read_project_any, write_folder, Project};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage:
  gmk-art build GAME [--only NAMES] [--dry-run]
      render GAME/art-src/**/*.art and write the sprites and backgrounds into the game
      folder (new ones are added; code, rooms and objects are untouched)
  gmk-art sheet GAME|FILE.art ... [-o sheet.png] [--only NAMES] [--detail] [--zoom N]
                [--bg SCENE] [--width PX]
      contact sheet PNG(s): each sprite against the game's backdrop, light, dark, as a
      silhouette with mask and origin, at 1x; --detail adds frame-to-frame motion
  gmk-art check GAME|FILE.art ... [--only NAMES] [--bg SCENE] [--verbose] [--json]
      contrast, palette, stray pixels, soft edges, outlines, pixel size, stale bboxes,
      flip/rotate origins, wrap seams and unbuilt .art files

  GAME      a project folder (or .gm81/.gmk for sheet and check)
  NAMES     comma-separated resource names
  SCENE     #rrggbb, a background name or a room name (default: the room the sprite
            is placed in, else the largest room)";

struct Opts {
    targets: Vec<String>,
    out: Option<PathBuf>,
    only: Vec<String>,
    detail: bool,
    zoom: Option<i32>,
    bg: Option<String>,
    width: Option<i32>,
    verbose: bool,
    json: bool,
    dry_run: bool,
}

fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts { targets: vec![], out: None, only: vec![], detail: false, zoom: None, bg: None, width: None, verbose: false, json: false, dry_run: false };
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let mut val = || -> Result<String, String> {
            i += 1;
            args.get(i).cloned().ok_or(format!("{a} needs a value"))
        };
        match a {
            "-o" | "--out" => o.out = Some(PathBuf::from(val()?)),
            "--only" => o.only.extend(val()?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty())),
            "--detail" => o.detail = true,
            "--zoom" => o.zoom = Some(val()?.parse().map_err(|_| "--zoom needs a number")?),
            "--width" => o.width = Some(val()?.parse().map_err(|_| "--width needs a number")?),
            "--bg" => o.bg = Some(val()?),
            "--verbose" | "-v" => o.verbose = true,
            "--json" => o.json = true,
            "--dry-run" => o.dry_run = true,
            _ if a.starts_with('-') => return Err(format!("unknown option {a}")),
            _ => o.targets.push(a.to_string()),
        }
        i += 1;
    }
    if o.targets.is_empty() {
        return Err("no game or .art file given".into());
    }
    Ok(o)
}

/// What a command works on: a game (with its `art-src`, if any) or loose `.art` files.
struct Target {
    project: Option<Project>,
    set: Option<ArtSet>,
}

fn art_src(dir: &Path) -> Option<PathBuf> {
    let d = dir.join("art-src");
    d.is_dir().then_some(d)
}

fn load_target(targets: &[String]) -> Result<Target, String> {
    let paths: Vec<PathBuf> = targets.iter().map(PathBuf::from).collect();
    if paths.iter().all(|p| p.extension().is_some_and(|e| e == "art")) {
        return Ok(Target { project: None, set: Some(load_set(&paths)?) });
    }
    if paths.len() != 1 {
        return Err("give one game, or only .art files".into());
    }
    let p = &paths[0];
    let project = read_project_any(p).map_err(|e| format!("{}: {e}", shown(p)))?;
    let set = match (p.is_dir(), art_src(p)) {
        (true, Some(d)) => Some(load_set(&art_files(&d))?),
        _ => None,
    };
    Ok(Target { project: Some(project), set })
}

fn print_warnings(set: &ArtSet) {
    for w in &set.warnings {
        eprintln!("warning: {w}");
    }
}

/// Items with the scene each one is judged against.
fn items_and_scenes(t: &Target, o: &Opts) -> Result<(Vec<Item>, Vec<Option<Scene>>), String> {
    let mut items = match &t.project {
        Some(p) => game::project_items(p, t.set.as_ref()),
        None => game::set_items(t.set.as_ref().unwrap()),
    };
    if !o.only.is_empty() {
        for n in &o.only {
            if !items.iter().any(|i| &i.name == n) {
                return Err(format!("`{n}` is not a sprite or background here"));
            }
        }
        items.retain(|i| o.only.contains(&i.name));
    }
    let forced = o.bg.as_deref().map(|s| game::named_scene(t.project.as_ref(), s)).transpose()?;
    let usage = t.project.as_ref().map(game::usage);
    let scenes = items
        .iter()
        .map(|it| {
            if !it.is_sprite {
                return None;
            }
            game::scene_for(t.project.as_ref(), it, usage.as_ref().and_then(|u| u.get(&it.name)), forced.as_ref())
        })
        .collect();
    Ok((items, scenes))
}

fn cmd_build(o: &Opts) -> Result<(), String> {
    if o.targets.len() != 1 {
        return Err("build takes one game folder".into());
    }
    let dir = PathBuf::from(&o.targets[0]);
    if !dir.join("project.toml").is_file() {
        return Err(format!("{} is not a project folder (no project.toml)", shown(&dir)));
    }
    let src = art_src(&dir).ok_or(format!("{} has no art-src folder", shown(&dir)))?;
    let files = art_files(&src);
    if files.is_empty() {
        return Err(format!("no .art files in {}", shown(&src)));
    }
    let set = load_set(&files)?;
    print_warnings(&set);
    // objects and rooms may already refer to resources that only exist in art-src:
    // give those a first image so the folder can be read
    let mut added = Vec::new();
    let mut p = match read_folder(&dir) {
        Ok(p) => p,
        Err(first) => {
            if o.dry_run {
                return Err(format!("{}: {first}", shown(&dir)));
            }
            added = game::add_missing(&dir, &set)?;
            if added.is_empty() {
                return Err(format!("{}: {first}", shown(&dir)));
            }
            read_folder(&dir).map_err(|e| format!("{}: {e}", shown(&dir)))?
        }
    };
    let mut changes = game::apply(&mut p, &set, &o.only)?;
    for (name, ch) in &mut changes {
        if added.contains(name) {
            *ch = Change::Added;
        }
    }
    let width = changes.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
    let mut changed = 0;
    for (name, ch) in &changes {
        let what = match ch {
            Change::Added => "added",
            Change::Updated => "updated",
            Change::Unchanged => "unchanged",
        };
        if *ch != Change::Unchanged {
            changed += 1;
        }
        let Some(r) = set.resources.iter().find(|r| &r.name == name) else {
            println!("{name:width$}  {what:9}");
            continue;
        };
        let f = &r.frames[0];
        let frames = if r.frames.len() == 1 { String::new() } else { format!(", {} frames", r.frames.len()) };
        println!("{name:width$}  {what:9}  {}x{}{frames}  ({}:{})", f.w, f.h, shown(&r.file), r.line);
    }
    if o.dry_run {
        println!("dry run: nothing written ({changed} would change)");
    } else if changed > 0 {
        write_folder(&p, &dir).map_err(|e| format!("{}: {e}", shown(&dir)))?;
        println!("wrote {changed} resource(s) into {}", shown(&dir));
    } else {
        println!("everything up to date");
    }
    Ok(())
}

fn cmd_sheet(o: &Opts) -> Result<(), String> {
    let t = load_target(&o.targets)?;
    if let Some(s) = &t.set {
        print_warnings(s);
    }
    let (items, scenes) = items_and_scenes(&t, o)?;
    if items.is_empty() {
        return Err("nothing to show".into());
    }
    let pairs: Vec<(Item, Scene)> = items
        .into_iter()
        .zip(scenes)
        .map(|(it, sc)| {
            let sc = sc.unwrap_or_else(|| Scene::plain("grey", instance_art::color::rgb(96, 96, 104)));
            (it, sc)
        })
        .collect();
    let opts = sheet::Options {
        zoom: o.zoom,
        detail: o.detail || (!o.only.is_empty() && o.only.len() <= 3),
        width: o.width.unwrap_or(1400),
        ..sheet::Options::default()
    };
    let pages = sheet::render(&pairs, &opts);
    let out = o.out.clone().unwrap_or_else(|| PathBuf::from("sheet.png"));
    for (i, page) in pages.iter().enumerate() {
        let path = if i == 0 {
            out.clone()
        } else {
            let stem = out.file_stem().unwrap_or_default().to_string_lossy();
            out.with_file_name(format!("{stem}-{}.png", i + 1))
        };
        sheet::write_png(page, &path)?;
        println!("{}  ({}x{})", shown(&path), page.w, page.h);
    }
    Ok(())
}

fn cmd_check(o: &Opts) -> Result<bool, String> {
    let t = load_target(&o.targets)?;
    let (mut items, mut scenes) = items_and_scenes(&t, o)?;
    // pieces are drawing helpers, not resources of the game
    if let (None, Some(set)) = (&t.project, &t.set) {
        let pieces: Vec<&str> = set.resources.iter().filter(|r| r.kind == Kind::Piece).map(|r| r.name.as_str()).collect();
        let keep: Vec<bool> = items.iter().map(|i| !pieces.contains(&i.name.as_str())).collect();
        let mut k = keep.iter();
        items.retain(|_| *k.next().unwrap());
        let mut k = keep.iter();
        scenes.retain(|_| *k.next().unwrap());
    }
    let usage = t.project.as_ref().map(game::usage);
    let rep = check::run(&check::Input { items: &items, scenes: &scenes, project: t.project.as_ref(), usage: usage.as_ref(), set: t.set.as_ref() });
    let mut findings = rep.findings.clone();
    if !o.only.is_empty() {
        findings.retain(|f| o.only.contains(&f.name));
    }
    if o.json {
        let v = serde_json::json!({ "findings": findings, "stats": rep.stats, "art_warnings": t.set.as_ref().map(|s| s.warnings.clone()).unwrap_or_default() });
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
        return Ok(findings.iter().all(|f| f.level != Level::Warning));
    }
    if let Some(s) = &t.set {
        print_warnings(s);
    }
    if o.verbose {
        println!("{:<20} {:>9} {:>3} {:>4} {:>3} {:>8} {:>7} {:>6}  scene", "name", "size", "frm", "cols", "px", "outline", "contr.", "blend");
        for s in &rep.stats {
            let dash = || "-".to_string();
            println!(
                "{:<20} {:>9} {:>3} {:>4} {:>3} {:>8} {:>7} {:>6}  {}",
                s.name,
                format!("{}x{}", s.width, s.height),
                s.frames,
                s.colours,
                s.pixel_scale.map(|k| format!("{k}x")).unwrap_or_else(dash),
                s.outline.clone().unwrap_or_else(dash),
                s.edge_contrast.map(|c| format!("{c:.2}")).unwrap_or_else(dash),
                s.blend.map(|b| format!("{:.0}%", b * 100.0)).unwrap_or_else(dash),
                s.scene.clone().unwrap_or_else(|| s.seam.map(|(h, v)| format!("seam {h:.1}/{v:.1}")).unwrap_or_default())
            );
        }
        println!();
    }
    let width = findings.iter().map(|f| f.name.len()).max().unwrap_or(0);
    for f in &findings {
        let lv = match f.level {
            Level::Warning => "warning",
            Level::Note => "note",
        };
        println!("{:width$}  {lv:7}  {}: {}", f.name, f.code, f.message);
    }
    let warnings = findings.iter().filter(|f| f.level == Level::Warning).count();
    let notes = findings.len() - warnings;
    let sprites = items.iter().filter(|i| i.is_sprite).count();
    println!("{warnings} warning(s), {notes} note(s) in {sprites} sprite(s) and {} background(s)", items.len() - sprites);
    Ok(warnings == 0)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    if cmd == "-h" || cmd == "--help" || cmd == "help" {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let o = match parse_opts(&args[1..]) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("gmk-art: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = match cmd.as_str() {
        "build" => cmd_build(&o).map(|_| true),
        "sheet" => cmd_sheet(&o).map(|_| true),
        "check" => cmd_check(&o),
        _ => Err(format!("unknown command `{cmd}`\n\n{USAGE}")),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("gmk-art: {e}");
            ExitCode::from(1)
        }
    }
}
