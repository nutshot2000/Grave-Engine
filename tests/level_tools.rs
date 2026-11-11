//! Level shapes (slope, hill, curve, poly, cover) and anchored scenery.

use grave_engine::{Materials, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

/// The first non-air row in a column.
fn top(w: &grave_engine::World, x: usize) -> usize {
    (0..w.h).find(|&y| w.mat[w.idx(x, y)] != 0).unwrap_or(w.h)
}

#[test]
fn shapes_fill_solid_ground_under_their_surface() {
    let m = mats();
    let text = "size 300 100\nfill air\nslope stone 0 90 100 60\nhill sandstone 100 200 30 90 cover sand 4\ncurve stone 200 90 250 90 299 50\npoly brick 10 10 40 10 25 30\n";
    let l = scene::load(text, &m, None);
    assert!(l.problems.is_empty(), "{:?}", l.problems);
    let w = &l.world;
    assert_eq!(top(w, 0), 90);
    assert!((top(w, 99) as i32 - 60).abs() <= 1, "the slope rises to 60: {}", top(w, 99));
    assert!((top(w, 150) as i32 - 60).abs() <= 1, "the hill's top is 30 above its base: {}", top(w, 150));
    let sand = m.id("sand").unwrap();
    assert_eq!(w.mat[w.idx(150, top(w, 150) + 1)], sand, "covered in sand");
    assert_eq!(w.mat[w.idx(150, top(w, 150) + 6)], m.id("sandstone").unwrap(), "sandstone under the cover");
    assert!(top(w, 220) > 75 && top(w, 290) < 60, "the curve bends up at the end");
    assert_eq!(w.mat[w.idx(25, 15)], m.id("brick").unwrap(), "inside the triangle");
    assert_eq!(w.mat[w.idx(12, 28)], 0, "outside the triangle");
    // Filled all the way down.
    assert_eq!(w.mat[w.idx(50, 99)], m.id("stone").unwrap());
}

#[test]
fn anchored_scenery_never_collapses_and_survives_saving() {
    let m = mats();
    // A stone beam sticking 70 cells out from a pillar: past stone's strength.
    let beam = "size 200 100
fill air
rect stone 0 90 200 10
rect stone 0 30 4 60
rect stone 4 30 70 4
";
    let pieces = |extra: &str| {
        let (mut s, p) = scene::load(&format!("{beam}{extra}"), &m, None).into_sim(&m);
        assert!(p.is_empty(), "{p:?}");
        for _ in 0..30 {
            s.step(&m);
        }
        (s.physics.body_count(), s)
    };
    let (loose, _) = pieces("");
    let (pinned, mut s) = pieces("anchor 4 30 70 4
");
    assert!(loose > 0, "an over-long beam snaps");
    assert_eq!(pinned, 0, "an anchored beam stays");
    let text = scene::save(&s.snapshot(), &m);
    assert!(text.contains("anchor 4 30 70 1"), "anchors are saved");
}
