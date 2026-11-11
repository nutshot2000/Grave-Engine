//! The material set: every material is complete, and the newer ones behave
//! like what they're named after.

use grave_engine::material::State;
use grave_engine::{Materials, Sim, scene};

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
    s.world.mat.iter().filter(|&&c| c == id).count()
}

#[test]
fn every_material_is_complete() {
    let m = mats();
    for mat in &m.list {
        if mat.name == "air" {
            continue;
        }
        assert!(!mat.about.is_empty(), "{} says what it is", mat.name);
        if mat.state == State::Solid && mat.name != "flesh" {
            assert!(mat.toughness.is_some(), "{} can be cracked", mat.name);
            assert!(mat.texture.is_some() || ["plastic"].contains(&mat.name.as_str()), "{} has a texture", mat.name);
        }
        if mat.metal && mat.state == State::Solid {
            assert!(mat.yield_strength.is_some() && mat.softens_at.is_some(), "metal {} bends and softens", mat.name);
            assert!(mat.melts.is_some(), "metal {} melts", mat.name);
        }
    }
    for name in ["steel", "aluminium", "copper", "glass", "concrete", "snow", "mud", "plastic", "cloth"] {
        assert!(m.id(name).is_some(), "there's {name}");
    }
}

/// Drop a stone block on a pane of `pane`; how many cells of the pane are left?
fn smash(m: &Materials, pane: &str) -> (usize, usize) {
    let mut s = sim(m, &format!("size 100 100\nfill air\nrect stone 0 90 100 10\nrect stone 30 60 4 30\nrect stone 66 60 4 30\nrect {pane} 34 60 32 2\nbody rect stone 44 20 10 10\n"));
    // (Only what's still in the pane: broken-off shards fly about as objects.)
    let in_pane = |s: &Sim| {
        let id = m.id(pane).unwrap();
        (0..s.world.mat.len()).filter(|&i| s.world.mat[i] == id && !s.world.is_owned(i)).count()
    };
    let before = in_pane(&s);
    for _ in 0..90 {
        s.step(m);
    }
    (before, in_pane(&s))
}

#[test]
fn glass_shatters_where_steel_holds() {
    let m = mats();
    let (glass, glass_after) = smash(&m, "glass");
    let (steel, steel_after) = smash(&m, "steel");
    println!("glass {glass} -> {glass_after}, steel {steel} -> {steel_after}");
    assert!(glass_after < glass, "the glass broke");
    assert_eq!(steel_after, steel, "the steel held");
}

#[test]
fn soft_copper_bends_further_than_steel() {
    let m = mats();
    let bend = |metal: &str| {
        let mut s = sim(&m, &format!("size 200 140\nfill air\nrect stone 0 130 200 10\nrect stone 112 90 4 40\nbody rect {metal} 60 60 60 2\ndepth 0.1\n"));
        let id = s.physics.ids()[0];
        s.physics.edit(&mut s.world, &m, id, |b| b.vy = 400.0);
        for _ in 0..40 {
            s.step(&m);
        }
        s.physics.bent(id)
    };
    let (copper, steel) = (bend("copper"), bend("steel"));
    println!("copper {copper} steel {steel}");
    assert!(copper > steel, "copper {copper} vs steel {steel}");
}

#[test]
fn aluminium_melts_where_steel_stays_solid() {
    let m = mats();
    let mut s = sim(&m, "size 60 40\nfill air\nrect stone 0 30 60 10\nrect aluminium 10 26 10 4\nrect steel 40 26 10 4\nheat rect 10 26 10 4 900\nheat rect 40 26 10 4 900\n");
    for _ in 0..3 {
        s.step(&m);
    }
    assert!(count(&s, &m, "molten_aluminium") > 0, "aluminium melts at 900 degrees (and sets again as the stone draws the heat off)");
    assert_eq!(count(&s, &m, "molten_steel"), 0, "steel doesn't");
}

#[test]
fn dirt_soaks_up_water_into_mud_and_snow_melts() {
    let m = mats();
    let mut s = sim(&m, "size 60 40\nfill air\nrect stone 0 30 60 10\nrect dirt 5 24 20 6\nrect water 5 20 20 4\nrect snow 40 26 10 4\n");
    for _ in 0..600 {
        s.step(&m);
    }
    assert!(count(&s, &m, "mud") > 10, "mud: {}", count(&s, &m, "mud"));
    assert!(count(&s, &m, "snow") < 40, "snow melts on a warm day");
    let (mud, sand) = (m.get(m.id("mud").unwrap()), m.get(m.id("sand").unwrap()));
    assert!(mud.rolling_resistance > sand.rolling_resistance, "mud bogs wheels down more than sand");
}
