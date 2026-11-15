//! Big worlds: millions of cells must load and run without crawling, and
//! sharing work between CPU cores must not change the answer.
use grave_engine::{Materials, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn sim(text: &str, m: &Materials) -> grave_engine::Sim {
    let (s, problems) = scene::load(text, m, None).into_sim(m);
    assert!(problems.is_empty(), "{problems:?}");
    s
}

/// The same hot spot spreads its heat exactly the same in a small world
/// (worked on by one core) and a big one (shared between cores).
#[test]
fn heat_is_the_same_on_one_core_or_many() {
    let m = mats();
    let spot = "rect stone 40 40 48 48\nheat circle 64 64 6 600\n";
    let mut small = sim(&format!("size 128 128\nfill air\n{spot}"), &m);
    let mut big = sim(&format!("size 1024 640\nfill air\n{spot}"), &m);
    for _ in 0..12 {
        small.step(&m);
        big.step(&m);
    }
    let mut hottest = 0.0f32;
    for y in 44..84 {
        for x in 44..84 {
            let (a, b) = (small.world.temp[small.world.idx(x, y)], big.world.temp[big.world.idx(x, y)]);
            assert_eq!(a.to_bits(), b.to_bits(), "temperature at ({x}, {y}): {a} on one core, {b} shared");
            hottest = hottest.max(a);
        }
    }
    assert!(hottest > 200.0, "the hot spot went cold ({hottest})");
}

/// A big world that's just sky and rock goes quiet soon after loading
/// instead of working on every cell for a second.
#[test]
fn a_calm_big_world_settles_quickly() {
    let m = mats();
    let mut s = sim("size 2048 512\nfill air\nrect rock 0 300 2048 212\n", &m);
    for _ in 0..20 {
        s.step(&m);
    }
    assert!(s.world.awake_fraction() < 0.02, "still {:.0}% awake", s.world.awake_fraction() * 100.0);
}

/// ...but anything that can move still gets its full time after a load:
/// sand pours and water spreads in a big, otherwise calm world.
#[test]
fn moving_things_still_move_after_loading() {
    let m = mats();
    let mut s = sim("size 2048 512\nfill air\nrect rock 0 300 2048 212\nrect sand 1000 200 20 20\nrect water 1300 280 20 20\n", &m);
    let sand = m.id("sand").unwrap();
    let water = m.id("water").unwrap();
    let count = |s: &grave_engine::Sim, mat, y0: usize, y1: usize, x0: usize, x1: usize| {
        (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).filter(|&(x, y)| s.world.mat[s.world.idx(x, y)] == mat).count()
    };
    for _ in 0..240 {
        s.step(&m);
    }
    assert_eq!(count(&s, sand, 200, 280, 1000, 1020), 0, "the sand never fell");
    assert!(count(&s, sand, 280, 300, 960, 1060) > 300, "the sand isn't in a pile on the rock");
    assert!(count(&s, water, 296, 300, 1240, 1400) > 50, "the water didn't spread along the rock");
}

/// Even a big world that has just woken up runs at a playable speed.
#[test]
fn a_big_world_loads_without_crawling() {
    let m = mats();
    let mut s = sim("size 4096 768\nfill air\nrect rock 0 500 4096 268\nrect water 2000 470 80 30\n", &m);
    let start = std::time::Instant::now();
    for _ in 0..60 {
        s.step(&m);
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / 60.0;
    // About 3 million cells. A generous bound (debug builds and busy test
    // machines): the point is that it isn't hundreds of ms per frame.
    if cfg!(not(debug_assertions)) {
        assert!(ms < 60.0, "{ms:.1} ms per frame for the first second");
    }
}
