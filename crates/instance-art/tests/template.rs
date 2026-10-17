//! End to end on the template game: build its `.art` sources into a copy of the
//! folder, read it back, check it, and make a contact sheet.

use instance_art::art::{art_files, load_set};
use instance_art::check;
use instance_art::game::{self, Change};
use instance_art::sheet;
use instance_format::{read_folder, write_folder};
use std::path::{Path, PathBuf};

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        let t = to.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &t);
        } else {
            std::fs::copy(&p, &t).unwrap();
        }
    }
}

fn template() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../games/_template")
}

#[test]
fn template_builds_checks_and_draws() {
    let dir = std::env::temp_dir().join(format!("instance-art-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(&template(), &dir);

    // the committed sprites are up to date with art-src
    let set = load_set(&art_files(&dir.join("art-src"))).unwrap();
    assert!(set.warnings.is_empty(), "{:?}", set.warnings);
    let mut p = read_folder(&dir).unwrap();
    let changes = game::apply(&mut p, &set, &[]).unwrap();
    assert!(changes.iter().all(|(_, c)| *c == Change::Unchanged), "{changes:?}");

    // a new sprite is added to the folder and survives a round trip
    std::fs::write(dir.join("art-src/extra.art"), "sprite spr_extra 4x4\norigin center\nrect 0 0 4 4 #ff0000\n").unwrap();
    let set = load_set(&art_files(&dir.join("art-src"))).unwrap();
    let changes = game::apply(&mut p, &set, &[]).unwrap();
    assert!(changes.contains(&("spr_extra".to_string(), Change::Added)), "{changes:?}");
    write_folder(&p, &dir).unwrap();
    let back = read_folder(&dir).unwrap();
    let s = back.sprites.iter().flatten().find(|s| s.name == "spr_extra").unwrap();
    assert_eq!((s.origin_x, s.origin_y, s.frames.len()), (2, 2, 1));
    assert!(std::fs::read_to_string(dir.join("resource_tree.txt")).unwrap().contains("spr_extra"));

    // checks: the template's art has no warnings
    let items = game::project_items(&back, Some(&set));
    let usage = game::usage(&back);
    let scenes: Vec<_> = items.iter().map(|it| game::scene_for(Some(&back), it, usage.get(&it.name), None)).collect();
    let rep = check::run(&check::Input { items: &items, scenes: &scenes, project: Some(&back), usage: Some(&usage), set: Some(&set) });
    let warnings: Vec<_> = rep.findings.iter().filter(|f| f.level == check::Level::Warning).collect();
    assert!(warnings.is_empty(), "{warnings:?}");
    // the player is placed in rm_game, so it is judged against that room
    let player = rep.stats.iter().find(|s| s.name == "spr_player").unwrap();
    assert!(player.scene.as_deref().unwrap().starts_with("rm_game"), "{:?}", player.scene);

    // a sheet renders
    let pairs: Vec<_> = items.into_iter().zip(scenes).map(|(i, s)| (i, s.unwrap())).collect();
    let pages = sheet::render(&pairs, &sheet::Options::default());
    assert_eq!(pages.len(), 1);
    assert!(pages[0].w > 200 && pages[0].h > 100);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn build_creates_resources_that_objects_already_use() {
    let dir = std::env::temp_dir().join(format!("instance-art-test-new-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(&template(), &dir);
    // an object written before its sprite exists: the folder cannot be read yet
    std::fs::create_dir_all(dir.join("objects/obj_rock")).unwrap();
    std::fs::write(dir.join("objects/obj_rock/object.toml"), "sprite = \"spr_rock\"\n").unwrap();
    std::fs::write(
        dir.join("art-src/rock.art"),
        "sprite spr_rock 8x8\norigin center\ncircle 4 4 3.5 #808080\nbackground bg_sky 16x16\nrect 0 0 16 16 #102040\n",
    )
    .unwrap();
    assert!(read_folder(&dir).is_err());
    let set = load_set(&art_files(&dir.join("art-src"))).unwrap();
    let added = game::add_missing(&dir, &set).unwrap();
    assert_eq!(added, vec!["spr_rock".to_string(), "bg_sky".to_string()]);
    let mut p = read_folder(&dir).unwrap();
    game::apply(&mut p, &set, &[]).unwrap();
    write_folder(&p, &dir).unwrap();
    let back = read_folder(&dir).unwrap();
    let s = back.sprites.iter().flatten().find(|s| s.name == "spr_rock").unwrap();
    assert_eq!((s.origin_x, s.frames[0].width), (4, 8));
    let rock = back.objects.iter().flatten().find(|o| o.name == "obj_rock").unwrap();
    assert_eq!(back.sprites[rock.sprite as usize].as_ref().unwrap().name, "spr_rock");
    // nothing more to add the second time
    assert!(game::add_missing(&dir, &set).unwrap().is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}
