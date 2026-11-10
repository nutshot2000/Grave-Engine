//! Templates: reusable objects with per-copy changes.

use grave_engine::template::{self, Change, Templates};
use grave_engine::edit::Transform;
use grave_engine::{Materials, scene};
use std::path::{Path, PathBuf};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

/// A private templates folder for one test.
fn folder(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("grave_templates_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("templates")).unwrap();
    d
}

#[test]
fn the_starter_templates_load() {
    let m = mats();
    let t = Templates::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/templates"), &m);
    assert!(t.problems.is_empty(), "{:?}", t.problems);
    for name in ["crate", "plank", "cart"] {
        assert!(t.get(name).is_some(), "missing {name}");
    }
}

#[test]
fn placed_copies_with_their_own_changes() {
    let m = mats();
    let dir = folder("place");
    std::fs::write(dir.join("templates/beam.txt"), "body rect wood 0 0 10 2\n").unwrap();
    let text = "size 100 60\nfill air\nrect stone 0 50 100 10\nplace beam 20 20\nplace beam 60 20 material iron flip scale 2\n";
    let loaded = scene::load(text, &m, Some(&dir));
    assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
    let (mut s, _) = loaded.into_sim(&m);
    assert_eq!(s.instances.len(), 2);
    let iron = m.id("iron").unwrap();
    let specs = s.physics.export(&mut s.world);
    let big = specs.iter().find(|b| b.w == 20).expect("the scaled copy is twice as wide");
    assert!(big.cells.iter().flatten().all(|c| c.mat == iron));
    let changes = s.instances.values().map(|i| i.changes.clone()).find(|c| !c.is_empty()).unwrap();
    assert_eq!(changes, vec![Change::Material("iron".into()), Change::Transform(Transform::FlipH), Change::Transform(Transform::Scale(2.0))]);
}

#[test]
fn editing_the_template_updates_every_copy_but_keeps_their_changes() {
    let m = mats();
    let dir = folder("refresh");
    let file = dir.join("templates/beam.txt");
    std::fs::write(&file, "body rect wood 0 0 10 2\n").unwrap();
    let text = "size 100 60\nfill air\nplace beam 20 20\nplace beam 60 20 material iron\n";
    let (mut s, p) = scene::load(text, &m, Some(&dir)).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    // Make the template longer.
    std::fs::write(&file, "body rect wood 0 0 16 2\n").unwrap();
    let t = Templates::load_dir(dir.join("templates"), &m);
    assert_eq!(template::refresh(&mut s, &t, &m), 2);
    let specs = s.physics.export(&mut s.world);
    assert!(specs.iter().all(|b| b.w == 16), "both copies got longer");
    let iron = m.id("iron").unwrap();
    assert!(specs.iter().any(|b| b.cells.iter().flatten().all(|c| c.mat == iron)), "the iron copy stayed iron");
}

#[test]
fn copies_save_as_one_line_and_load_back() {
    let m = mats();
    // `save` looks for templates in ./templates, which is the project folder in tests.
    let base = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/scenes"));
    let (mut s, p) = scene::load("size 100 60\nfill air\nplace plank 50 20 angle 30 material iron\n", &m, Some(base)).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    let saved = scene::save(&s.snapshot(), &m);
    assert!(saved.contains("place plank 50.00 20.00 angle 30.0 material iron"), "{saved}");
    let (s2, p2) = scene::load(&saved, &m, Some(base)).into_sim(&m);
    assert!(p2.is_empty(), "{p2:?}");
    assert_eq!(s2.instances.len(), 1);
}

#[test]
fn a_damaged_copy_is_saved_as_its_own_shape() {
    let m = mats();
    let base = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/scenes"));
    let (mut s, _) = scene::load("size 100 60\nfill air\nplace plank 50 20\n", &m, Some(base)).into_sim(&m);
    // Knock a hole in it.
    s.world.set(&m, 50, 20, 0);
    s.step(&m);
    assert!(s.instances.is_empty(), "a damaged copy is unlinked from its template");
    let saved = scene::save(&s.snapshot(), &m);
    assert!(!saved.contains("place plank"), "and saved as its own shape");
}
