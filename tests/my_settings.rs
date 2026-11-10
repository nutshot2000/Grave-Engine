//! Your own material settings, laid over the defaults, and resetting them.

use grave_engine::Materials;
use std::path::{Path, PathBuf};

/// A private copy of the materials folder to play with.
fn copy_materials(tag: &str) -> PathBuf {
    let src = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"));
    let dst = std::env::temp_dir().join(format!("grave_mats_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(dst.join("textures")).unwrap();
    for e in std::fs::read_dir(src).unwrap().flatten() {
        if e.path().is_file() {
            std::fs::copy(e.path(), dst.join(e.file_name())).unwrap();
        }
    }
    dst
}

#[test]
fn your_settings_change_a_material_and_reset_brings_back_the_defaults() {
    let dir = copy_materials("a");
    let mut m = Materials::load_dir(&dir);
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    let sand = m.id("sand").unwrap();
    let (friction0, thirst0) = (m.get(sand).friction, m.get(sand).reactions[0].chance);

    m.save_my_settings("sand", &[("friction".into(), "0.1".into()), ("reaction".into(), "water -> wet_sand + air, chance 0.9".into())]).unwrap();
    assert!(m.changed_on_disk(), "the editor notices");
    m.reload();
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    let s = m.get(sand);
    assert_eq!(s.friction, 0.1);
    assert_eq!(s.reactions.len(), 1, "a changed reaction replaces the old one");
    assert_eq!(s.reactions[0].chance, 0.9);
    assert_eq!(s.custom, vec!["friction".to_string(), "reaction:water".to_string()]);
    // The defaults are still known, and the material's own file is untouched.
    assert_eq!(m.default_of(sand).friction, friction0);
    assert_eq!(m.default_of(sand).reactions[0].chance, thirst0);
    assert!(std::fs::read_to_string(dir.join("sand.txt")).unwrap().contains("friction = 0.7"));
    // Other materials aren't affected.
    let stone = m.id("stone").unwrap();
    assert!(m.get(stone).custom.is_empty());

    // Reset all: the settings file goes, and so do the changes.
    m.save_my_settings("sand", &[]).unwrap();
    m.reload();
    assert_eq!(m.get(sand).friction, friction0);
    assert_eq!(m.get(sand).reactions[0].chance, thirst0);
    assert!(m.get(sand).custom.is_empty());
    assert!(!dir.join("my_settings").join("sand.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
