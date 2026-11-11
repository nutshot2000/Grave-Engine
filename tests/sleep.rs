//! Sleeping areas: quiet parts of the world are skipped, and wake the moment
//! anything happens near them.

use grave_engine::{Materials, Sim, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn sim(m: &Materials, text: &str) -> Sim {
    let (s, p) = scene::load(text, m, None).into_sim(m);
    assert!(p.is_empty(), "{p:?}");
    s
}

fn count(s: &Sim, m: &Materials, name: &str, x0: usize, x1: usize, y0: usize, y1: usize) -> usize {
    let id = m.id(name).unwrap();
    (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).filter(|&(x, y)| s.world.mat[s.world.idx(x, y)] == id).count()
}

#[test]
fn a_quiet_world_goes_to_sleep_and_wakes_when_its_support_goes() {
    let m = mats();
    // A big still world with a sand pile sitting on a stone shelf.
    let mut s = sim(&m, "size 600 400\nfill air\nrect stone 0 380 600 20\nrect stone 300 200 40 4\nrect sand 305 180 30 20\n");
    for _ in 0..200 {
        s.step(&m);
    }
    assert!(s.world.awake_fraction() < 0.2, "mostly asleep: {}", s.world.awake_fraction());
    let on_shelf = count(&s, &m, "sand", 300, 340, 150, 200);
    assert!(on_shelf > 250, "the sand sits on the shelf: {on_shelf}");
    // Knock the shelf away (as an object hitting it would): the sand falls.
    for y in 200..204 {
        for x in 300..340 {
            let i = s.world.idx(x, y);
            s.world.make_air(i);
        }
    }
    for _ in 0..240 {
        s.step(&m);
    }
    let fallen = count(&s, &m, "sand", 250, 400, 340, 380);
    assert!(fallen > 400, "the sand fell to the floor: {fallen}");
}

/// Run a scene with and without sleeping areas: `look` measures something.
fn both(m: &Materials, text: &str, setup: impl Fn(&mut Sim), frames: usize, look: impl Fn(&Sim) -> f32) -> (f32, f32) {
    let run = |sleep: bool| {
        let mut s = sim(m, text);
        s.world.sleeping = sleep;
        for _ in 0..150 {
            s.step(m);
        }
        setup(&mut s);
        for _ in 0..frames {
            s.step(m);
        }
        look(&s)
    };
    (run(true), run(false))
}

#[test]
fn heat_creeps_along_iron_as_far_as_without_sleeping() {
    let m = mats();
    let (sleep, awake) = both(
        &m,
        "size 600 100
fill air
rect stone 0 90 600 10
rect iron 20 50 500 3
",
        |s| {
            for y in 50..53 {
                for x in 20..30 {
                    let i = s.world.idx(x, y);
                    s.world.temp[i] = 1200.0;
                    s.world.wake_at(i);
                }
            }
        },
        900,
        |s| s.world.temp[s.world.idx(45, 51)],
    );
    println!("15 cells along: {sleep} with sleeping, {awake} without");
    assert!((sleep - awake).abs() < 1.0 + (awake - 20.0).abs() * 0.1, "sleeping {sleep} vs awake {awake}");
}

#[test]
fn fire_spreads_along_a_beam_as_with_everything_awake() {
    let m = mats();
    let wood = |s: &Sim| {
        let id = m.id("wood").unwrap();
        s.world.mat.iter().filter(|&&c| c == id).count() as f32
    };
    let (sleep, awake) = both(
        &m,
        "size 600 100
fill air
rect stone 0 90 600 10
rect wood 20 80 500 3
",
        |s| {
            for y in 80..83 {
                for x in 20..24 {
                    let i = s.world.idx(x, y);
                    s.world.temp[i] = 600.0;
                    s.world.wake_at(i);
                }
            }
        },
        900,
        wood,
    );
    println!("wood left: {sleep} with sleeping, {awake} without");
    assert!(awake < 1500.0 - 50.0, "the fire did spread");
    assert!((sleep - awake).abs() < 0.1 * (1500.0 - awake) + 20.0, "sleeping {sleep} vs awake {awake}");
}

#[test]
fn a_thrown_object_lands_in_sleeping_sand_like_in_awake_sand() {
    let m = mats();
    let (sleep, awake) = both(
        &m,
        "size 400 200
fill air
rect stone 0 190 400 10
rect sand 0 160 400 30
body rect iron 20 20 8 8
",
        |s| {
            let id = s.physics.ids()[0];
            s.physics.edit(&mut s.world, &m, id, |b| {
                b.x = 300.0;
                b.y = 100.0;
                b.vy = 300.0;
            });
        },
        150,
        |s| s.physics.spec_of(&s.world, s.physics.ids()[0]).unwrap().y,
    );
    println!("block ends at y {sleep} with sleeping, {awake} without");
    assert!(sleep > 150.0 && (sleep - awake).abs() < 3.0);
}
