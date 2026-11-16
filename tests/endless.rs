//! Endless worlds (src/endless.rs): the world scrolls over an endless map,
//! saving what leaves and loading or generating what comes in.
use grave_engine::endless::Endless;
use grave_engine::levelgen::{self, Settings};
use grave_engine::{Materials, Sim, World, scene};
use std::path::PathBuf;

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("grave-endless-test-{name}-{}", std::process::id()))
}

/// Any part of the map comes out the same, however it's cut up: generating
/// region by region matches generating the whole area at once.
#[test]
fn the_map_is_the_same_however_it_is_cut_up() {
    let m = mats();
    let s = Settings { open_sides: true, seed: 5, ..Settings::new(&m) };
    let origin = (-1000, 300);
    let mut whole = World::new(768, 512);
    levelgen::generate_at(&mut whole, &m, (0, 0, 768, 512), origin, &s, &[]);
    let mut parts = World::new(768, 512);
    for ry in (0..2).rev() {
        for rx in [2, 0, 1] {
            levelgen::generate_at(&mut parts, &m, (rx * 256, ry * 256, 256, 256), origin, &s, &[]);
        }
    }
    let differ = whole.mat.iter().zip(&parts.mat).filter(|(a, b)| a != b).count();
    assert_eq!(differ, 0, "{differ} cells differ between whole and region-by-region");
}

/// Authored landmarks and rounded terrain also agree across arbitrary cuts.
#[test]
fn rounded_tiles_match_across_negative_map_coordinates_and_partial_bricks() {
    let m = mats();
    let (tiles, problems) = levelgen::Tile::load_dir(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tiles/caves"), &m);
    assert!(problems.is_empty());
    let settings = Settings { tile: 47, open_sides: true, seed: 21, ..Settings::new(&m) };
    let origin = (-173, -91);
    let mut whole = World::new(384, 256);
    levelgen::generate_at(&mut whole, &m, (0, 0, 384, 256), origin, &settings, &tiles);
    let mut parts = World::new(384, 256);
    for ry in 0..2 { for rx in 0..3 {
        levelgen::generate_at(&mut parts, &m, (rx * 128, ry * 128, 128, 128), origin, &settings, &tiles);
    } }
    assert_eq!(whole.mat, parts.mat, "rounding and landmarks must not depend on region boundaries");
    assert_eq!(whole.back, parts.back);
}

fn endless_sim(m: &Materials) -> (Sim, Endless) {
    let text = "size 768 768\nfill air\nendless seed 9 pools 0\n";
    let (sim, problems) = scene::load(text, m, None).into_sim(m);
    assert!(problems.is_empty(), "{problems:?}");
    let mut e = Endless::from_scene(text, m, None, "test").unwrap();
    e.dir = dir("walk");
    e.new_run();
    (sim, e)
}

#[test]
fn walking_right_scrolls_and_new_caves_join_on() {
    let m = mats();
    let (mut sim, mut e) = endless_sim(&m);
    // Something at the right edge: the world scrolls left by one region.
    assert_eq!(e.update(&mut sim, &m, (700.0, 400.0)), Some((-256, 0)));
    assert_eq!(e.origin, (256, 0));
    assert_eq!(e.stats, (3, 0, 3), "three regions saved, three generated");
    // The new strip is exactly what the map has there.
    let mut fresh = World::new(256, 768);
    levelgen::generate_at(&mut fresh, &m, (0, 0, 256, 768), (256 + 512, 0), &e.settings, &e.tiles);
    for y in 0..768 {
        for x in 0..256 {
            assert_eq!(sim.world.mat[sim.world.idx(512 + x, y)], fresh.mat[fresh.idx(x, y)], "cell ({x}, {y}) of the new strip");
        }
    }
    // The middle of the window is calm: no scrolling.
    assert_eq!(e.update(&mut sim, &m, (400.0, 400.0)), None);
    let _ = std::fs::remove_dir_all(&e.dir);
}

#[test]
fn going_back_finds_things_where_they_were_left() {
    let m = mats();
    let (mut sim, mut e) = endless_sim(&m);
    e.dir = dir("back");
    e.new_run();
    // Dig a mark into the left region and drop a crate there.
    for y in 300..310 {
        for x in 100..140 {
            sim.world.set(&m, x, y, m.id("brick").unwrap());
        }
    }
    let spec = grave_engine::physics::BodySpec::from_mats(6, 6, &vec![Some(m.id("wood").unwrap()); 36], &m, 120.0, 250.0);
    sim.physics.add(&mut sim.world, &m, spec);
    let detail = sim.world.idx(100, 300);
    sim.world.tex[detail] = 1234;
    sim.world.depth_air[detail] = true;
    let bodies = sim.physics.body_count();
    // Go right (the left region is saved away), then come back.
    e.update(&mut sim, &m, (700.0, 400.0));
    assert_eq!(sim.physics.body_count(), bodies - 1, "the crate was saved with its region");
    e.update(&mut sim, &m, (100.0, 400.0));
    assert_eq!(e.origin, (0, 0));
    assert_eq!(e.stats.1, 3, "the old regions came back from disk");
    let brick = m.id("brick").unwrap();
    assert!((300..310).all(|y| (100..140).all(|x| sim.world.mat[sim.world.idx(x, y)] == brick)), "the brick mark is still there");
    assert_eq!(sim.physics.body_count(), bodies, "the crate came back");
    assert_eq!(sim.world.tex[detail], 1234);
    assert!(sim.world.depth_air[detail]);
    e.settle();
    let _ = std::fs::remove_dir_all(&e.dir);
}

#[test]
fn the_player_keeps_their_place_on_the_map() {
    let m = mats();
    let text = "size 768 768\nfill air\nendless seed 4 pools 0\nrect air 650 380 60 40\nrect rock 650 420 60 8\nplayer 680 420\n";
    let (mut sim, problems) = scene::load(text, &m, None).into_sim(&m);
    assert!(problems.is_empty(), "{problems:?}");
    let mut e = Endless::from_scene(text, &m, None, "player").unwrap();
    e.dir = dir("player");
    e.new_run();
    let before = sim.physics.actor_status()[0].x;
    let shift = e.update(&mut sim, &m, (before, 400.0)).unwrap();
    let after = sim.physics.actor_status()[0].x;
    assert_eq!(after - before, shift.0 as f32, "the player moved with the world");
    assert_eq!(after as i64 + e.origin.0, before as i64, "same place on the map");
    for _ in 0..30 {
        sim.step(&m);
    }
    assert!(!sim.physics.actor_status()[0].dead);
    let _ = std::fs::remove_dir_all(&e.dir);
}

#[test]
fn scrolling_is_quick_enough() {
    let m = mats();
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/scenes/endless-caves.txt")).unwrap();
    let (mut sim, problems) = scene::load(&text, &m, Some(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/scenes")))).into_sim(&m);
    assert!(problems.is_empty(), "{problems:?}");
    let mut e = Endless::from_scene(&text, &m, Some(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/scenes"))), "quick").unwrap();
    e.dir = dir("quick");
    e.new_run();
    assert!(e.tiles.len() > 20, "the demo uses the hand-made tiles");
    for _ in 0..30 {
        sim.step(&m);
    }
    let t = std::time::Instant::now();
    e.update(&mut sim, &m, (1200.0, 500.0)).expect("scrolled");
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    eprintln!("one scroll of a 1280x1024 world: {ms:.1} ms");
    if cfg!(not(debug_assertions)) {
        assert!(ms < 400.0, "a scroll took {ms:.0} ms");
    }
    let _ = std::fs::remove_dir_all(&e.dir);
}

#[test]
#[ignore = "timing breakdown: cargo test --release --test endless profile -- --ignored --nocapture"]
fn profile_scrolling() {
    let m = mats();
    let base = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/scenes"));
    let text = std::fs::read_to_string(base.join("endless-caves.txt")).unwrap();
    let (mut sim, _) = scene::load(&text, &m, Some(base)).into_sim(&m);
    let mut e = Endless::from_scene(&text, &m, Some(base), "profile").unwrap();
    e.dir = dir("profile");
    e.new_run();
    for _ in 0..30 {
        sim.step(&m);
    }
    grave_engine::profile::enable(true);
    for k in 0..4 {
        // Walk up to the edge like a player: the strip ahead gets made first.
        let (near, edge) = if k % 2 == 0 { (900.0, 1200.0) } else { (380.0, 80.0) };
        assert!(e.update(&mut sim, &m, (near, 500.0)).is_none());
        for _ in 0..30 {
            sim.step(&m);
        }
        e.update(&mut sim, &m, (edge, 500.0)).unwrap();
        for f in 0..20 {
            let t = std::time::Instant::now();
            sim.step(&m);
            if f < 3 || f == 19 {
                eprintln!("  frame {f} after scroll {k}: {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
            }
        }
    }
    grave_engine::profile::enable(false);
    eprintln!("{}", grave_engine::profile::report());
    let _ = std::fs::remove_dir_all(&e.dir);
}

/// Sliding the world in place keeps everything running: a thrown crate
/// keeps its speed, a crate resting on the floor stays resting (the rock
/// outlines moved with it), the player keeps standing.
#[test]
fn a_smooth_scroll_keeps_physics_running() {
    let m = mats();
    let text = "size 1024 512\nfill air\nrect stone 0 400 1024 112\nbody rect wood 600 380 12 12\nplayer 700 400\n";
    let (mut sim, problems) = scene::load(text, &m, None).into_sim(&m);
    assert!(problems.is_empty(), "{problems:?}");
    let wood = m.id("wood").unwrap();
    let spec = grave_engine::physics::BodySpec { vx: 40.0, ..grave_engine::physics::BodySpec::from_mats(6, 6, &vec![Some(wood); 36], &m, 500.0, 200.0) };
    let thrown = sim.physics.add(&mut sim.world, &m, spec).unwrap();
    let resting = sim.physics.ids()[0];
    for _ in 0..90 {
        sim.step(&m);
    }
    let (rx, ry, _) = sim.physics.pose_of(resting).unwrap();
    let (tx, _, _) = sim.physics.pose_of(thrown).unwrap();
    sim.shift(-256, 0);
    let (rx2, ry2, _) = sim.physics.pose_of(resting).unwrap();
    assert!((rx2 - (rx - 256.0)).abs() < 0.01 && (ry2 - ry).abs() < 0.01, "moved exactly with the world");
    for _ in 0..120 {
        sim.step(&m);
    }
    let (rx3, ry3, _) = sim.physics.pose_of(resting).unwrap();
    assert!((rx3 - rx2).abs() < 1.0 && (ry3 - ry2).abs() < 1.0, "the resting crate stayed put: ({rx2}, {ry2}) -> ({rx3}, {ry3})");
    let (tx2, _, _) = sim.physics.pose_of(thrown).unwrap();
    assert!(tx2 < tx - 256.0 + 120.0 && tx2 > tx - 256.0 - 5.0, "the thrown crate carried on from where it was: {tx} -> {tx2}");
    let a = &sim.physics.actor_status()[0];
    assert!(!a.dead && a.grounded, "the player is still standing");
    // Stone that slid in from the right is solid ground too.
    assert_eq!(sim.world.mat[sim.world.idx(100, 450)], m.id("stone").unwrap());
}

/// Bent (natural-looking) caves join seamlessly too: the bend uses map
/// coordinates and each region is carved with a margin around it.
#[test]
fn bent_caves_are_the_same_however_they_are_cut_up() {
    let m = mats();
    let (tiles, _) = levelgen::Tile::load_dir(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tiles/caves"), &m);
    let s = Settings { tile: 140, bend: 1.0, open_sides: true, seed: 33, ..Settings::new(&m) };
    let origin = (-300, 170);
    let mut whole = World::new(768, 512);
    levelgen::generate_at(&mut whole, &m, (0, 0, 768, 512), origin, &s, &tiles);
    let mut parts = World::new(768, 512);
    for ry in 0..2 {
        for rx in 0..3 {
            levelgen::generate_at(&mut parts, &m, (rx * 256, ry * 256, 256, 256), origin, &s, &tiles);
        }
    }
    let differ = whole.mat.iter().zip(&parts.mat).filter(|(a, b)| a != b).count();
    assert_eq!(differ, 0, "{differ} cells differ between whole and region-by-region");
    // And it really did bend: not the same as the unbent map.
    let mut flat = World::new(768, 512);
    levelgen::generate_at(&mut flat, &m, (0, 0, 768, 512), origin, &Settings { bend: 0.0, ..s.clone() }, &tiles);
    assert!(whole.mat.iter().zip(&flat.mat).filter(|(a, b)| a != b).count() > 5000);
}

/// Dangers (gas, flooded dips, loose gravel) land in the same places
/// however the map is cut up, and there are some of each.
#[test]
fn cave_dangers_are_the_same_however_the_map_is_cut_up() {
    let m = mats();
    let s = Settings { tile: 140, bend: 1.0, dangers: 1.0, open_sides: true, seed: 33, ..Settings::new(&m) };
    let origin = (-300, 170);
    let mut whole = World::new(768, 512);
    levelgen::generate_at(&mut whole, &m, (0, 0, 768, 512), origin, &s, &[]);
    let mut parts = World::new(768, 512);
    for ry in 0..2 {
        for rx in 0..3 {
            levelgen::generate_at(&mut parts, &m, (rx * 256, ry * 256, 256, 256), origin, &s, &[]);
        }
    }
    let differ = whole.mat.iter().zip(&parts.mat).filter(|(a, b)| a != b).count();
    assert_eq!(differ, 0, "{differ} cells differ between whole and region-by-region");
    for name in ["volatile_gas", "water", "gravel"] {
        let id = m.id(name).unwrap();
        let n = whole.mat.iter().filter(|&&c| c == id).count();
        assert!(n > 20, "only {n} cells of {name}");
    }
}
