//! Bad or silly input shouldn't hang or crash the engine (agents write
//! scenes too, and typos happen).

use grave_engine::{Materials, World, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

#[test]
fn huge_numbers_in_a_scene_load_quickly() {
    let m = mats();
    let text = "size 200 100\nfill air\n\
        rect stone -1000000000 90 2000000000 2000000000\n\
        circle sand 50 50 100000000\n\
        line wood -2000000000 0 2000000000 10 999999\n\
        heat rect 0 0 2000000000 2000000000 600\n\
        heat circle 100 50 2000000000 30\n\
        body rect wood 0 0 99999 99999\n\
        body circle stone 50 20 99999\n";
    let start = std::time::Instant::now();
    let loaded = scene::load(text, &m, None);
    assert!(start.elapsed().as_secs_f32() < 5.0, "took {:?}", start.elapsed());
    assert_eq!((loaded.world.w, loaded.world.h), (200, 100));
    assert!(loaded.bodies.iter().all(|b| b.w <= 1025 && b.h <= 1025));
}

#[test]
fn flying_sand_never_pops_out_through_a_roof() {
    let m = mats();
    let (stone, sand) = (m.id("stone").unwrap(), m.id("sand").unwrap());
    let mut w = World::new(60, 60);
    w.fill(&m, m.id("air").unwrap());
    // A stone roof (rows 20-39) over a cave packed with sand (rows 40-59).
    for y in 20..60 {
        for x in 0..60 {
            w.set(&m, x, y, if y < 40 { stone } else { sand });
        }
    }
    // Throw a grain gently upwards, then fill the gap it left (the pile
    // shifting), so it has nowhere to land nearby.
    assert!(w.throw(&m, 30, 50, 0.0, -0.5));
    w.set(&m, 30, 50, sand);
    for _ in 0..5 {
        w.step(&m);
    }
    let above = (0..20).flat_map(|y| (0..60).map(move |x| (x, y))).filter(|&(x, y)| w.mat[w.idx(x, y)] == sand).count();
    assert_eq!(above, 0, "no sand teleported above the roof");
}

#[test]
fn a_character_with_an_empty_animation_is_refused_not_a_crash() {
    let dir = std::env::temp_dir().join(format!("grave_look_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("character.txt"), "figure = 100\nfeet = 120\nidle = 0 frames, 8 fps, loop\n").unwrap();
    let r = grave_engine::look::Look::load(&dir, 16);
    assert!(r.is_err());
}
