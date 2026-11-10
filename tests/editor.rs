//! Editing operations and undo/redo.

use grave_engine::edit::{self, History, Region, Transform};
use grave_engine::sim::Sim;
use grave_engine::{Materials, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn sim(m: &Materials, text: &str) -> Sim {
    let (s, p) = scene::load(text, m, None).into_sim(m);
    assert!(p.is_empty(), "{p:?}");
    s
}

fn count(s: &Sim, m: &Materials, name: &str) -> usize {
    let id = m.id(name).unwrap();
    s.world.mat.iter().filter(|&&x| x == id).count()
}

#[test]
fn undo_and_redo_painting() {
    let m = mats();
    let mut s = sim(&m, "size 40 30\nfill air\nrect stone 0 25 40 5\n");
    let mut h = History::new();
    h.touch(&mut s, "paint");
    s.world.paint_circle(&m, 20, 10, 3, m.id("sand").unwrap());
    h.commit(&mut s);
    let sand = count(&s, &m, "sand");
    assert!(sand > 20);
    assert_eq!(h.undo(&mut s, &m).as_deref(), Some("paint"));
    assert_eq!(count(&s, &m, "sand"), 0);
    h.redo(&mut s, &m);
    assert_eq!(count(&s, &m, "sand"), sand);
}

#[test]
fn many_undo_steps() {
    let m = mats();
    let mut s = sim(&m, "size 40 30\nfill air\n");
    let mut h = History::new();
    for x in 0..30 {
        h.touch(&mut s, "dot");
        s.world.set(&m, x, 5, m.id("stone").unwrap());
        h.commit(&mut s);
    }
    assert_eq!(h.len(), 30);
    for _ in 0..30 {
        h.undo(&mut s, &m);
    }
    assert_eq!(count(&s, &m, "stone"), 0);
    for _ in 0..30 {
        h.redo(&mut s, &m);
    }
    assert_eq!(count(&s, &m, "stone"), 30);
}

#[test]
fn moving_an_object_keeps_its_id_and_undoes() {
    let m = mats();
    let mut s = sim(&m, "size 60 40\nfill air\nrect stone 0 35 60 5\nbody rect wood 10 10 6 4\n");
    let id = s.physics.ids()[0];
    let mut h = History::new();
    h.touch(&mut s, "move");
    assert!(s.physics.edit(&mut s.world, &m, id, |b| {
        b.x = 40.0;
        b.y = 20.0;
    }));
    h.commit(&mut s);
    assert_eq!(s.physics.ids(), vec![id]);
    let spec = s.physics.spec_of(&s.world, id).unwrap();
    assert!((spec.x - 40.0).abs() < 0.01);
    assert_eq!(s.physics.id_at(&s.world, 40, 20), Some(id), "the object is drawn where it was moved");
    h.undo(&mut s, &m);
    let ids = s.physics.ids();
    let back = s.physics.spec_of(&s.world, ids[0]).unwrap();
    assert!((back.x - 13.0).abs() < 0.01, "undo puts it back, x={}", back.x);
}

#[test]
fn turning_and_flipping_are_reversible() {
    let m = mats();
    let mut s = sim(&m, "size 60 40\nfill air\nbody rect wood 10 10 6 2\nrect stone 30 10 5 3\nrect sand 30 13 2 1\n");
    let id = s.physics.ids()[0];
    let before = s.physics.spec_of(&s.world, id).unwrap();
    for _ in 0..4 {
        s.physics.edit(&mut s.world, &m, id, |b| edit::transform_body(b, Transform::Turn));
    }
    let after = s.physics.spec_of(&s.world, id).unwrap();
    assert_eq!((after.w, after.h), (before.w, before.h));
    let once = {
        s.physics.edit(&mut s.world, &m, id, |b| edit::transform_body(b, Transform::Turn));
        s.physics.spec_of(&s.world, id).unwrap()
    };
    assert_eq!((once.w, once.h), (2, 6), "a quarter turn swaps width and height");

    // Scenery: flip twice = unchanged.
    let r = Region::new(&s.world, 30, 10, 34, 13);
    let original = s.world.mat.clone();
    let r2 = edit::transform_region(&mut s.world, r, Transform::FlipH);
    edit::transform_region(&mut s.world, r2, Transform::FlipH);
    assert_eq!(s.world.mat, original);
}

#[test]
fn resizing_an_object() {
    let m = mats();
    let mut s = sim(&m, "size 80 60\nfill air\nbody rect wood 10 10 6 4\n");
    let id = s.physics.ids()[0];
    s.physics.edit(&mut s.world, &m, id, |b| edit::transform_body(b, Transform::Scale(2.0)));
    let b = s.physics.spec_of(&s.world, id).unwrap();
    assert_eq!((b.w, b.h), (12, 8));
    assert_eq!(b.cell_count(), 96);
}

#[test]
fn cut_and_paste_scenery() {
    let m = mats();
    let mut s = sim(&m, "size 60 40\nfill air\nrect stone 5 5 4 4\n");
    let r = Region::new(&s.world, 5, 5, 8, 8);
    let clip = edit::cut_region(&mut s.world, r);
    assert_eq!(count(&s, &m, "stone"), 0);
    edit::paste_clip(&mut s.world, &clip, 40, 20);
    assert_eq!(count(&s, &m, "stone"), 16);
    assert_eq!(s.world.mat[s.world.idx(41, 21)], m.id("stone").unwrap());
}

#[test]
fn removing_an_object() {
    let m = mats();
    let mut s = sim(&m, "size 60 40\nfill air\nbody rect iron 10 10 4 4\n");
    let id = s.physics.ids()[0];
    let spec = s.physics.remove(&mut s.world, id).unwrap();
    assert_eq!(spec.cell_count(), 16);
    assert_eq!(s.physics.body_count(), 0);
    assert_eq!(count(&s, &m, "iron"), 0);
}

#[test]
fn rewind_goes_back_to_exactly_how_things_were() {
    let m = mats();
    let mut s = sim(
        &m,
        "size 120 80\nfill air\nrect stone 0 70 120 10\nrect wood 20 40 60 4\nrect wood 20 44 4 26\nrect wood 76 44 4 26\nbody rect wood 40 10 10 10\nheat rect 20 40 6 4 600\n",
    );
    let mut t = edit::Timeline::new();
    let wood = m.id("wood").unwrap();
    let mut seen = Vec::new(); // (frame, wood cells, objects) at each recorded moment
    t.record(&mut s);
    seen.push((s.world.frame, s.world.mat.iter().filter(|&&x| x == wood).count(), s.physics.body_count()));
    for _ in 0..600 {
        s.step(&m);
        let before = t.len();
        t.record(&mut s);
        if t.len() > before {
            seen.push((s.world.frame, s.world.mat.iter().filter(|&&x| x == wood).count(), s.physics.body_count()));
        }
    }
    assert_eq!(t.len(), seen.len());
    assert!(seen.first().unwrap().1 > seen.last().unwrap().1 + 50, "the fire should have burned some wood");
    for i in [0, 3, seen.len() / 2, seen.len() - 1] {
        assert!(t.restore(i, &mut s, &m));
        let w = s.world.mat.iter().filter(|&&x| x == wood).count();
        assert_eq!((s.world.frame, w, s.physics.body_count()), seen[i], "moment {i}");
    }
    // Carry on from the middle: the later moments are replaced.
    let mid = seen.len() / 2;
    t.restore(mid, &mut s, &m);
    t.truncate_after(mid, &mut s);
    assert_eq!(t.len(), mid + 1);
    for _ in 0..30 {
        s.step(&m);
        t.record(&mut s);
    }
    assert!(t.len() > mid + 1);
}
