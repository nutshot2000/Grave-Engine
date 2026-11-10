//! Sand soaks up water and turns into wet sand, which clumps and dries.

use grave_engine::{Materials, scene};

fn mats() -> Materials {
    let m = Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"));
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    m
}

fn count(w: &grave_engine::World, m: &Materials, name: &str) -> usize {
    w.census(m).into_iter().find(|(n, _, _)| n == name).map_or(0, |(_, c, _)| c)
}

#[test]
fn sand_soaks_up_water_and_the_water_is_used_up() {
    let m = mats();
    let mut w = scene::load("size 100 60\nfill air\nrect stone 0 55 100 5\nrect sand 0 35 100 20\nrect water 0 25 100 10\n", &m, None).world;
    let (water0, wet0) = (count(&w, &m, "water"), count(&w, &m, "wet_sand"));
    for _ in 0..600 {
        w.step(&m);
    }
    let (water1, wet1) = (count(&w, &m, "water"), count(&w, &m, "wet_sand"));
    assert_eq!(wet0, 0);
    assert!(water1 < water0 - 100, "water soaked in: {water0} -> {water1}");
    // Every drop that went in made one cell of wet sand (a bit may have dried at the surface).
    assert!(wet1 > 0 && wet1 <= water0 - water1, "wet sand {wet1}, water used {}", water0 - water1);
}

#[test]
fn wet_sand_holds_its_shape_and_dry_sand_slumps() {
    let m = mats();
    let height = |name: &str| {
        let mut w = scene::load(&format!("size 60 60\nfill air\nrect stone 0 55 60 5\nrect {name} 25 15 10 40\n"), &m, None).world;
        for _ in 0..600 {
            w.step(&m);
        }
        (0..55).find(|&y| (0..60).any(|x| w.mat[w.idx(x, y)] != 0)).map_or(0, |top| 55 - top)
    };
    let (wet, dry) = (height("wet_sand"), height("sand"));
    assert!(wet >= 35, "a wet sand tower stands: {wet} tall");
    assert!(dry <= 25, "a dry sand tower slumps: {dry} tall");
}

#[test]
fn heat_dries_wet_sand() {
    let m = mats();
    let mut w = scene::load("size 40 40\nfill air\nrect wet_sand 10 20 20 10\nheat rect 10 20 20 10 150\n", &m, None).world;
    w.step(&m);
    assert_eq!(count(&w, &m, "wet_sand"), 0);
    assert!(count(&w, &m, "sand") > 150);
}
