//! Wet and oily ground (src/world/coating.rs): liquids leave a film on solid
//! ground that dries over time, grips less, and (oil) burns.
use grave_engine::{Materials, Sim, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn sim(text: &str, m: &Materials) -> Sim {
    let (s, problems) = scene::load(text, m, None).into_sim(m);
    assert!(problems.is_empty(), "{problems:?}");
    s
}

/// Wet cells on the floor's top row, x0..x1.
fn wet_floor(s: &Sim, m: &Materials, y: usize, x0: usize, x1: usize) -> usize {
    (x0..x1).filter(|&x| s.world.wetness(m, s.world.idx(x, y)) > 0.0).count()
}

#[test]
fn water_leaves_the_ground_wet_and_it_dries() {
    let m = mats();
    // A splash of water on a tilted stone floor: it runs off the edge.
    let mut s = sim("size 200 80\nfill air\nslope stone 0 50 200 70\nrect water 20 40 10 8\n", &m);
    for _ in 0..240 {
        s.step(&m);
    }
    let surface = |s: &Sim, x: usize| (0..80).find(|&y| s.world.mat[s.world.idx(x, y)] != 0).unwrap();
    let wet = (0..200).filter(|&x| s.world.wetness(&m, s.world.idx(x, surface(&s, x))) > 0.0).count();
    assert!(wet > 40, "the water's path should be wet: {wet} cells");
    // Water dries in about 45 seconds.
    for _ in 0..(60 * 50) {
        s.step(&m);
    }
    let water = m.id("water").unwrap();
    let still_water = s.world.mat.iter().filter(|&&c| c == water).count();
    let wet_now = (0..200).filter(|&x| {
        let i = s.world.idx(x, surface(&s, x));
        s.world.wetness(&m, i) > 0.0 && ![i - s.world.w, i - 1, i + 1].iter().any(|&j| s.world.mat[j] == water)
    }).count();
    assert_eq!(wet_now, 0, "dry after a minute (water left lying: {still_water} cells)");
}

/// How far a wooden box slides down a gentle stone slope in 3 seconds.
fn slide(film: &str) -> f32 {
    let m = mats();
    let mut s = sim(&format!("size 260 120\nfill air\nslope stone 0 60 260 112\n{film}body rect wood 30 52 10 5\n"), &m);
    let id = s.physics.ids()[0];
    for _ in 0..30 {
        s.step(&m);
    }
    let start = s.physics.pose_of(id).unwrap().0;
    for _ in 0..180 {
        s.step(&m);
    }
    s.physics.pose_of(id).unwrap().0 - start
}

#[test]
fn things_slide_on_an_oil_slick() {
    let dry = slide("");
    let oily = slide("coat oil 0 50 260 70 forever\n");
    let wet = slide("coat water 0 50 260 70 forever\n");
    assert!(oily > dry + 20.0, "oil should send the box sliding: dry {dry:.1}, oily {oily:.1}");
    assert!(wet >= dry, "wet ground can't grip better than dry: dry {dry:.1}, wet {wet:.1}");
}

#[test]
fn a_film_slowly_loses_its_slipperiness() {
    let m = mats();
    let mut s = sim("size 100 40\nfill air\nrect stone 0 30 100 10\ncoat water 0 30 100 1\n", &m);
    let i = s.world.idx(50, 30);
    assert_eq!(s.world.coat_level(&m, i), 3, "soaked at first");
    for _ in 0..(60 * 20) {
        s.step(&m);
    }
    let mid = s.world.coat_level(&m, i);
    assert!(mid == 2 || mid == 1, "drying after 20 s: level {mid}");
    for _ in 0..(60 * 30) {
        s.step(&m);
    }
    assert_eq!(s.world.coat_level(&m, i), 0, "dry after 50 s");
}

#[test]
fn an_oil_slick_catches_fire() {
    let m = mats();
    let mut s = sim("size 160 60\nfill air\nrect stone 0 50 160 10\ncoat oil 10 50 140 1\nheat circle 20 50 3 400\n", &m);
    let fire = m.id("fire").unwrap();
    let mut flames = 0;
    for _ in 0..600 {
        s.step(&m);
        flames = flames.max(s.world.mat.iter().filter(|&&c| c == fire).count());
    }
    assert!(flames > 5, "the slick never burned");
    assert!(wet_floor(&s, &m, 50, 10, 150) < 100, "the fire should have burned along the slick");
}

#[test]
fn water_does_not_wash_away_oil_and_lava_leaves_no_film() {
    let m = mats();
    let oil = m.id("oil").unwrap();
    let mut s = sim("size 100 40\nfill air\nrect stone 0 30 100 10\ncoat oil 0 30 100 1 forever\nrect water 40 26 10 4\n", &m);
    for _ in 0..60 {
        s.step(&m);
    }
    assert_eq!(s.world.coat[s.world.idx(45, 30)], oil, "the oil is still under the water");
    assert!(!m.get(m.id("lava").unwrap()).leaves_film());
    assert!(m.get(m.id("water").unwrap()).leaves_film());
}

#[test]
fn films_are_saved_with_the_scene() {
    let m = mats();
    let mut s = sim("size 100 40\nfill air\nrect stone 0 30 100 10\ncoat oil 10 30 30 1 forever\ncoat water 60 30 20 1\n", &m);
    let text = scene::save(&s.snapshot(), &m);
    assert!(text.contains("coat oil 10 30 30 1 forever"), "{text}");
    assert!(text.contains("coat water 60 30 20 1"), "{text}");
    let again = sim(&text, &m);
    assert_eq!(wet_floor(&again, &m, 30, 0, 100), 50);
}

#[test]
fn coat_needs_a_liquid() {
    let m = mats();
    let (_, problems) = scene::load("size 50 20\nfill air\ncoat stone 0 0 10 10\n", &m, None).into_sim(&m);
    assert!(!problems.is_empty());
}

