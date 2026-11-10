//! Rigid objects and importing, checked against the real material files.

use grave_engine::sim::Sim;
use grave_engine::{Materials, import, scene};
use std::path::Path;

fn mats() -> Materials {
    let m = Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"));
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    m
}

fn sim(m: &Materials, text: &str) -> Sim {
    let (sim, problems) = scene::load(text, m, Some(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/scenes")))).into_sim(m);
    assert!(problems.is_empty(), "{problems:?}");
    sim
}

fn count(sim: &Sim, m: &Materials, name: &str) -> usize {
    let id = m.id(name).unwrap();
    sim.world.mat.iter().filter(|&&x| x == id).count()
}

#[test]
fn objects_fall_and_land_on_the_ground() {
    let m = mats();
    let mut s = sim(&m, "size 60 60\nfill air\nrect stone 0 50 60 10\nbody rect stone 20 5 8 8\n");
    for _ in 0..240 {
        s.step(&m);
    }
    let b = s.physics.export(&mut s.world);
    assert_eq!(b.len(), 1);
    // Centre should be half a block (4 cells) above the ground at y=50.
    assert!((b[0].y - 46.0).abs() < 1.0, "landed at y={}", b[0].y);
    assert!(b[0].vy.abs() < 1.0, "still moving: {}", b[0].vy);
    assert_eq!(count(&s, &m, "stone"), 600 + 64, "object cells should be drawn into the grid");
}

#[test]
fn cutting_an_object_splits_it() {
    let m = mats();
    let mut s = sim(&m, "size 60 40\nfill air\nrect stone 0 30 60 10\nbody rect wood 10 25 40 4\n");
    for _ in 0..30 {
        s.step(&m);
    }
    // Erase a slice through the middle, like the Erase tool does.
    for y in 0..30 {
        for x in 29..32 {
            s.world.set(&m, x, y, 0);
        }
    }
    s.step(&m);
    assert_eq!(s.physics.body_count(), 2);
}

#[test]
fn wood_floats_and_iron_sinks() {
    let m = mats();
    let mut s = sim(
        &m,
        "size 80 80\nfill air\nrect stone 0 70 80 10\nrect stone 0 0 2 70\nrect stone 78 0 2 70\nrect water 2 30 76 40\nbody rect wood 10 10 20 4\nbody rect iron 50 10 6 6\n",
    );
    let water_before = count(&s, &m, "water");
    for _ in 0..600 {
        s.step(&m);
    }
    let b = s.physics.export(&mut s.world);
    let wood = b.iter().find(|b| b.w == 20).unwrap();
    let iron = b.iter().find(|b| b.w == 6).unwrap();
    assert!(wood.y < 34.0, "wood should float near the surface, at y={}", wood.y);
    assert!(iron.y > 64.0, "iron should sink to the bottom, at y={}", iron.y);
    assert!(wood.angle.abs() < 0.2, "the plank should float flat, angle {}", wood.angle);
    assert_eq!(count(&s, &m, "water"), water_before, "no water should be lost");
}

#[test]
fn burning_objects_fall_apart() {
    let m = mats();
    // Plenty of open air: in a small sealed box the smoke would smother it.
    let mut s = sim(&m, "size 240 160\nfill air\nrect stone 0 150 240 10\nbody rect wood 100 120 40 6\nheat rect 100 120 40 6 600\n");
    let wood_before = count(&s, &m, "wood");
    for _ in 0..900 {
        s.step(&m);
    }
    let left = count(&s, &m, "wood");
    assert!(left < wood_before / 2, "most of the wood should have burned: {left} of {wood_before} left, {} objects", s.physics.body_count());
    assert!(count(&s, &m, "ash") > 0);
}

#[test]
fn make_objects_turns_painted_things_loose() {
    let m = mats();
    let mut s = sim(&m, "size 60 60\nfill air\nrect stone 0 50 60 10\nrect wood 10 10 10 4\nrect sand 40 10 4 4\n");
    let n = s.physics.make_objects(&mut s.world, &m, 0, 0, 59, 30);
    assert_eq!(n, 2);
    for _ in 0..200 {
        s.step(&m);
    }
    let b = s.physics.export(&mut s.world);
    assert!(b.iter().all(|b| b.y > 44.0), "both should have fallen: {:?}", b.iter().map(|b| b.y).collect::<Vec<_>>());
}

#[test]
fn objects_survive_save_and_load() {
    let m = mats();
    let mut s = sim(&m, "size 60 60\nfill air\nrect stone 0 50 60 10\nbody rect wood 10 10 10 4\nbody circle stone 40 20 4\n");
    for _ in 0..20 {
        s.step(&m);
    }
    let text = scene::save(&s.snapshot(), &m);
    let s2 = sim(&m, &text);
    assert_eq!(s2.physics.body_count(), 2);
    assert_eq!(count(&s2, &m, "wood"), count(&s, &m, "wood"));
}

#[test]
fn reset_restores_objects() {
    let m = mats();
    let mut s = sim(&m, "size 60 60\nfill air\nrect stone 0 50 60 10\nbody rect wood 10 10 10 4\n");
    let snap = s.snapshot();
    for _ in 0..100 {
        s.step(&m);
    }
    let back = Sim::restore(&snap, &m);
    let mut back = back;
    let b = back.physics.export(&mut back.world);
    assert!((b[0].y - 12.0).abs() < 0.5, "should be back at the start, y={}", b[0].y);
}

#[test]
fn png_colours_become_materials() {
    let m = mats();
    let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/crate.png"));
    let (shape, warnings) = import::load(path, &m, None).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!((shape.w, shape.h), (24, 24));
    let wood = shape.cells.iter().filter(|c| **c == m.id("wood")).count();
    let iron = shape.cells.iter().filter(|c| **c == m.id("iron")).count();
    assert!(wood > 200 && iron > 80, "wood {wood}, iron {iron}");
    assert_eq!(shape.cells.iter().filter(|c| c.is_none()).count(), 4, "transparent corners stay empty");
}

#[test]
fn blender_models_become_materials() {
    let m = mats();
    let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/cart.glb"));
    let (shape, warnings) = import::load(path, &m, None).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    // About 3.2 m wide at 10 cells per metre.
    assert!((30..=34).contains(&shape.w), "width {}", shape.w);
    for name in ["wood", "iron", "stone"] {
        let n = shape.cells.iter().filter(|c| **c == m.id(name)).count();
        assert!(n > 10, "{name}: {n} cells");
    }
}

#[test]
fn blender_material_names_are_forgiving() {
    let m = mats();
    assert_eq!(import::material_from_name("Wood.001", &m), m.id("wood"));
    assert_eq!(import::material_from_name("old_wood_planks", &m), m.id("wood"));
    assert_eq!(import::material_from_name("Molten Iron", &m), m.id("molten_iron"));
    assert_eq!(import::material_from_name("Plastic", &m), None);
}

#[test]
fn scenery_falls_when_its_support_is_gone() {
    let m = mats();
    // A post standing on the ground, holding up a plank.
    let mut s = sim(&m, "size 60 60\nfill air\nrect stone 0 50 60 10\nrect wood 28 20 4 30\nrect wood 10 16 40 4\n");
    s.step(&m);
    assert_eq!(s.physics.body_count(), 0, "everything is held up at the start");
    // Cut the post (like it burning through).
    for x in 28..32 {
        for y in 40..44 {
            s.world.set(&m, x, y, 0);
        }
    }
    for _ in 0..120 {
        s.step(&m);
    }
    assert_eq!(s.physics.body_count(), 1, "the top of the post and the plank should break loose together");
    let b = s.physics.export(&mut s.world);
    assert!(b[0].y > 30.0, "and fall, now at y={}", b[0].y);
}

#[test]
fn floating_scenery_stays_until_damaged() {
    let m = mats();
    let mut s = sim(&m, "size 60 60\nfill air\nrect stone 0 50 60 10\nrect stone 20 10 20 4\n");
    for _ in 0..30 {
        s.step(&m);
    }
    assert_eq!(s.physics.body_count(), 0, "a painted floating platform stays put");
    s.world.set(&m, 20, 10, 0);
    s.step(&m);
    assert_eq!(s.physics.body_count(), 1, "once damaged, it falls");
}

/// Count wood cells that are still fixed scenery (not falling objects).
fn fixed_wood(s: &Sim, m: &Materials) -> usize {
    let id = m.id("wood").unwrap();
    (0..s.world.mat.len()).filter(|&i| s.world.mat[i] == id && !s.world.is_owned(i)).count()
}

#[test]
fn short_overhangs_hold() {
    let m = mats();
    let strength = m.get(m.id("wood").unwrap()).strength as usize;
    // A wall with a shelf sticking out a bit less than wood's strength.
    let text = format!("size 120 60\nfill air\nrect stone 0 50 120 10\nrect wood 10 10 4 40\nrect wood 14 20 {} 3\n", strength - 5);
    let mut s = sim(&m, &text);
    for _ in 0..60 {
        s.step(&m);
    }
    assert_eq!(s.physics.body_count(), 0);
}

#[test]
fn long_overhangs_snap_at_their_strength() {
    let m = mats();
    let strength = m.get(m.id("wood").unwrap()).strength as usize;
    let reach = strength + 30;
    let text = format!("size 160 60\nfill air\nrect stone 0 50 160 10\nrect wood 10 10 4 40\nrect wood 14 20 {reach} 3\n");
    let mut s = sim(&m, &text);
    let before = fixed_wood(&s, &m);
    for _ in 0..60 {
        s.step(&m);
    }
    assert_eq!(s.physics.body_count(), 1, "the far end should snap off");
    // The wall and the first `strength` cells of the shelf stay.
    assert_eq!(fixed_wood(&s, &m), before - (reach - strength) * 3);
}

#[test]
fn burning_a_support_makes_the_overhang_snap() {
    let m = mats();
    // A beam held at both ends; each half is shorter than wood's strength.
    let strength = m.get(m.id("wood").unwrap()).strength as usize;
    let span = strength * 2 - 10;
    let text = format!("size 200 60\nfill air\nrect stone 0 50 200 10\nrect wood 10 10 4 40\nrect wood {} 10 4 40\nrect wood 14 10 {span} 3\n", 14 + span);
    let mut s = sim(&m, &text);
    for _ in 0..20 {
        s.step(&m);
    }
    assert_eq!(s.physics.body_count(), 0, "held at both ends, it stands");
    // Remove the right-hand post (as if it burned away).
    for y in 10..50 {
        for x in 14 + span..18 + span {
            s.world.set(&m, x, y, 0);
        }
    }
    for _ in 0..20 {
        s.step(&m);
    }
    assert!(s.physics.body_count() >= 1, "now it overhangs too far and snaps");
}

#[test]
fn falling_objects_crush_ash() {
    let m = mats();
    // A deep ash pile on the ground, and a wooden block dropped on it.
    let mut s = sim(&m, "size 60 80\nfill air\nrect stone 0 70 60 10\nrect ash 0 50 60 20\nbody rect wood 20 5 10 6\n");
    let ash_before = count(&s, &m, "ash");
    for _ in 0..300 {
        s.step(&m);
    }
    let b = s.physics.export(&mut s.world);
    assert!(b[0].y > 64.0, "the block should crush through the ash to the ground, at y={}", b[0].y);
    assert!(count(&s, &m, "ash") < ash_before - 40, "ash under it should be gone");
}

#[test]
fn sand_still_holds_objects_up() {
    let m = mats();
    let mut s = sim(&m, "size 60 80\nfill air\nrect stone 0 70 60 10\nrect sand 0 50 60 20\nbody rect wood 20 5 10 6\n");
    for _ in 0..300 {
        s.step(&m);
    }
    let b = s.physics.export(&mut s.world);
    assert!(b[0].y < 50.0, "the block should rest on top of the sand, at y={}", b[0].y);
}

#[test]
fn heavy_fast_things_smash_through_wood() {
    let m = mats();
    let bridge = "size 120 120\nfill air\nrect stone 0 110 120 10\nrect stone 20 60 10 50\nrect stone 90 60 10 50\nrect wood 30 60 60 4\n";
    // A light wooden block set down gently: the bridge holds.
    let mut gentle = sim(&m, &format!("{bridge}body rect wood 55 50 8 8\n"));
    let before = fixed_wood(&gentle, &m);
    for _ in 0..200 {
        gentle.step(&m);
    }
    assert_eq!(fixed_wood(&gentle, &m), before, "a gentle landing shouldn't break anything");
    // A heavy iron block dropped from high up: it smashes through.
    let mut smash = sim(&m, &format!("{bridge}body rect iron 52 2 14 14\n"));
    for _ in 0..200 {
        smash.step(&m);
    }
    assert!(fixed_wood(&smash, &m) < before - 20, "the bridge should be broken");
    let iron = smash.physics.export(&mut smash.world).into_iter().find(|b| b.w == 14).unwrap();
    assert!(iron.y > 90.0, "the iron block should end up on the ground, at y={}", iron.y);
}

#[test]
fn gunpowder_explodes_even_when_buried() {
    let m = mats();
    // Gunpowder buried in stone (no air), heated: it still goes off,
    // cracks the stone into gravel and throws the crate on top.
    let mut s = sim(
        &m,
        "size 120 120\nfill air\nrect stone 0 80 120 40\nrect gunpowder 50 88 20 8\nbody rect wood 52 70 16 10\nheat rect 50 88 20 8 400\n",
    );
    for _ in 0..3 {
        s.step(&m);
    }
    assert_eq!(count(&s, &m, "gunpowder"), 0, "it should all have gone off");
    assert!(count(&s, &m, "gravel") > 30, "the stone around it should crack into gravel");
    let fastest_up = s.physics.export(&mut s.world).into_iter().map(|b| b.vy).fold(0.0f32, f32::min);
    assert!(fastest_up < -20.0, "pieces of the crate should be thrown upward, fastest vy={fastest_up}");
}

#[test]
fn explosions_set_each_other_off() {
    let m = mats();
    let mut s = sim(&m, "size 120 60\nfill air\nrect stone 0 50 120 10\nrect gunpowder 20 44 6 6\nrect gunpowder 34 44 6 6\nheat rect 20 44 6 6 400\n");
    for _ in 0..30 {
        s.step(&m);
    }
    assert_eq!(count(&s, &m, "gunpowder"), 0, "the second pile should be set off by the first");
}

#[test]
fn ice_is_slippery() {
    let m = mats();
    // The same slope, once plain stone and once coated in ice. A wooden block
    // set at the top slides much further on the ice.
    let slope = |coat: &str| {
        let mut t = String::from("size 200 100\nambient -20\nfill air\nrect stone 0 90 200 10\n");
        for x in (10..150).step_by(2) {
            let top = 30 + (x - 10) * 30 / 140 + 1;
            t += &format!("rect stone {x} {top} 2 {}\n", 90 - top);
        }
        t += &format!("line {coat} 10 30 150 60 3\nheat rect 0 0 200 100 -20\nbody rect wood 14 18 8 8\n");
        let mut s = sim(&m, &t);
        for _ in 0..240 {
            s.step(&m);
        }
        s.physics.export(&mut s.world)[0].x
    };
    let (on_stone, on_ice) = (slope("stone"), slope("ice"));
    assert!(on_ice > on_stone + 40.0, "ice: {on_ice}, stone: {on_stone}");
}

#[test]
fn blasts_splinter_objects_too() {
    let m = mats();
    // A wooden box sitting right next to a gunpowder charge.
    let mut s = sim(
        &m,
        "size 120 80\nfill air\nrect stone 0 70 120 10\nrect gunpowder 40 64 20 6\nbody rect wood 42 44 16 20\nheat rect 40 64 20 6 400\n",
    );
    let wood_before = 16 * 20;
    for _ in 0..30 {
        s.step(&m);
    }
    let bodies = s.physics.export(&mut s.world);
    let biggest = bodies.iter().map(|b| b.cell_count()).max().unwrap_or(0);
    assert!(biggest < wood_before - 20, "the box should lose chunks, biggest piece {biggest} of {wood_before}");
    assert!(bodies.len() > 3, "and throw splinters: {} objects", bodies.len());
}

#[test]
fn objects_placed_inside_the_ground_pop_out() {
    let m = mats();
    let s = sim(&m, "size 100 60\nfill air\nrect stone 0 50 100 10\nbody rect stone 40 45 10 10\n");
    let mut s = s;
    let b = s.physics.export(&mut s.world);
    assert!(b[0].y <= 45.5, "should sit on top of the ground, centre y={}", b[0].y);
    assert_eq!(count(&s, &m, "stone"), 1000 + 100, "all of it should be visible");
}

#[test]
fn the_same_scene_always_plays_out_the_same() {
    // Agents rely on this: a headless run must be repeatable.
    let m = mats();
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/scenes/forces.txt")).unwrap();
    let run = || {
        let (mut s, _) = scene::load(&text, &m, Some(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/scenes")))).into_sim(&m);
        for _ in 0..700 {
            s.step(&m);
        }
        (s.world.mat.clone(), s.physics.body_count())
    };
    assert!(run() == run(), "two runs of the same scene differ");
}
