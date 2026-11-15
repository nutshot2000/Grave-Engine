//! Objects placed inside scenery are lifted clear, and the scene says so.
use grave_engine::{Materials, scene};

#[test]
fn an_object_inside_scenery_is_moved_up_and_reported() {
    let m = Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"));
    // A crate whose bottom half is inside a stone block.
    let text = "size 100 60\nfill air\nrect stone 0 50 100 10\nrect stone 40 40 20 10\nbody rect wood 45 36 10 8\n";
    let (s, problems) = scene::load(text, &m, None).into_sim(&m);
    assert_eq!(s.physics.body_count(), 1);
    assert!(problems.iter().any(|p| p.contains("moved up") && p.contains("wood")), "{problems:?}");
    // Resting on top is fine: no warning.
    let text = "size 100 60\nfill air\nrect stone 0 50 100 10\nbody rect wood 45 42 10 8\n";
    let (_, problems) = scene::load(text, &m, None).into_sim(&m);
    assert!(problems.is_empty(), "{problems:?}");
}
