//! Flying pixels: loose stuff thrown through the air by blasts.

use grave_engine::{Materials, Sim, World, edit, scene};
use std::path::Path;

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn sim(m: &Materials, text: &str) -> Sim {
    let (sim, problems) = scene::load(text, m, Some(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/scenes")))).into_sim(m);
    assert!(problems.is_empty(), "{problems:?}");
    sim
}

/// How many cells of a material there are, counting ones in the air.
fn total(w: &World, m: &Materials, name: &str) -> usize {
    w.census(m).into_iter().find(|(n, _, _)| n == name).map_or(0, |(_, c, _)| c)
}

#[test]
fn a_thrown_cell_flies_in_an_arc_and_lands() {
    let m = mats();
    let mut w = World::new(120, 60);
    w.fill(&m, m.id("air").unwrap());
    for x in 0..120 {
        w.set(&m, x, 59, m.id("stone").unwrap());
    }
    w.set(&m, 10, 58, m.id("sand").unwrap());
    assert!(w.throw(&m, 10, 58, 0.5, -1.5));
    assert_eq!(w.flying.len(), 1);
    assert_eq!(w.mat[w.idx(10, 58)], 0, "the cell leaves the grid while it flies");
    let mut top = 58.0f32;
    for _ in 0..300 {
        w.step(&m);
        if let Some(f) = w.flying.first() {
            top = top.min(f.y);
        }
    }
    assert!(w.flying.is_empty(), "it lands");
    assert!(top < 30.0, "it went up first: highest {top}");
    let sand = m.id("sand").unwrap();
    let landed = (0..120).find(|&x| w.mat[w.idx(x, 58)] == sand).expect("sand on the ground");
    assert!(landed > 40, "it flew a long way sideways: landed at {landed}");
}

/// (A closed box: things flung right out of the world are gone for good.)
#[test]
fn buried_charges_throw_the_pile_and_nothing_is_lost() {
    let m = mats();
    let text = "size 200 120\nfill air\nrect stone 0 110 200 10\nrect stone 0 0 2 110\nrect stone 198 0 2 110\nrect stone 0 0 200 2\n\
                rect sand 40 80 120 30\nrect gunpowder 96 100 8 6\nheat circle 100 103 2 600\n";
    let mut s = sim(&m, text);
    let sand0 = total(&s.world, &m, "sand");
    let mut most_flying = 0;
    let mut highest = 120usize;
    let sand = m.id("sand").unwrap();
    for _ in 0..240 {
        s.step(&m);
        most_flying = most_flying.max(s.world.flying.len());
        for f in &s.world.flying {
            if f.mat == sand {
                highest = highest.min(f.y.max(0.0) as usize);
            }
        }
    }
    assert!(most_flying > 200, "the blast throws lots of sand: {most_flying}");
    assert!(highest < 60, "sand from deep in the pile goes high: {highest}");
    let sand1 = total(&s.world, &m, "sand");
    assert_eq!(sand0, sand1, "every grain comes back down somewhere");
}

#[test]
fn rewind_remembers_what_was_in_the_air() {
    let m = mats();
    let text = "size 160 100\nfill air\nrect stone 0 90 160 10\nrect water 40 60 80 30\nrect gunpowder 76 82 8 6\nheat circle 80 85 2 600\n";
    let mut s = sim(&m, text);
    let mut t = edit::Timeline::new();
    t.every = 1;
    t.record(&mut s);
    let mut mid = None;
    for _ in 0..30 {
        s.step(&m);
        t.record(&mut s);
        if mid.is_none() && s.world.flying.len() > 50 {
            mid = Some((t.len() - 1, s.world.flying.clone()));
        }
    }
    let (at, flying) = mid.expect("water was thrown");
    assert!(t.restore(at, &mut s, &m));
    assert_eq!(s.world.flying, flying);
}

#[test]
fn objects_splash_when_they_hit_water_fast_but_not_when_slow() {
    let m = mats();
    let pool = "size 100 140\nfill air\nrect stone 0 130 100 10\nrect stone 20 90 4 40\nrect stone 76 90 4 40\nrect water 24 100 52 30\n";
    let most_water_flying = |text: &str| {
        let mut s = sim(&m, text);
        let water = m.id("water").unwrap();
        let mut most = 0;
        for _ in 0..200 {
            s.step(&m);
            most = most.max(s.world.flying.iter().filter(|f| f.mat == water).count());
        }
        most
    };
    let dropped = most_water_flying(&format!("{pool}body rect iron 44 5 10 10\n"));
    let lowered = most_water_flying(&format!("{pool}body rect iron 44 92 10 10\n"));
    assert!(dropped > 20, "a block dropped from high up splashes: {dropped}");
    assert_eq!(lowered, 0, "a block that just sinks in doesn't");
}
