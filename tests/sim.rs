//! Core simulation rules, checked against the real material files.

use grave_engine::{Materials, World, material::AIR, scene};

fn mats() -> Materials {
    let m = Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"));
    assert!(m.problems.is_empty(), "material files have problems: {:?}", m.problems);
    m
}

fn id(m: &Materials, name: &str) -> u16 {
    m.id(name).unwrap_or_else(|| panic!("no material {name}"))
}

/// A small world full of `fill`, with one cell of `centre` in the middle.
fn small(m: &Materials, fill: &str, centre: &str) -> (World, usize) {
    let mut w = World::new(21, 21);
    w.fill(m, id(m, fill));
    w.set(m, 10, 10, id(m, centre));
    let i = w.idx(10, 10);
    (w, i)
}

#[test]
fn all_material_files_load() {
    let m = mats();
    for name in ["air", "stone", "sand", "wood", "oil", "water", "ice", "steam", "fire", "smoke", "lava", "iron"] {
        assert!(m.id(name).is_some(), "missing {name}");
    }
}

#[test]
fn heat_spreads_through_iron() {
    let m = mats();
    let mut w = World::new(40, 3);
    w.fill(&m, id(&m, "iron"));
    let hot = w.idx(0, 1);
    let far = w.idx(20, 1);
    w.temp[hot] = 1000.0;
    for _ in 0..200 {
        w.temp[hot] = 1000.0; // keep one end hot, like a torch
        w.step(&m);
    }
    assert!(w.temp[w.idx(5, 1)] > 300.0, "near end only {}", w.temp[w.idx(5, 1)]);
    assert!(w.temp[far] > 25.0, "heat never reached the middle");
    assert!(w.temp[far] < w.temp[w.idx(5, 1)], "heat should fall off with distance");
}

#[test]
fn stone_conducts_slower_than_iron() {
    let m = mats();
    let run = |mat: &str| {
        let mut w = World::new(40, 3);
        w.fill(&m, id(&m, mat));
        for _ in 0..100 {
            let h = w.idx(0, 1);
            w.temp[h] = 1000.0;
            w.step(&m);
        }
        w.temp[w.idx(8, 1)]
    };
    assert!(run("iron") > run("stone") + 50.0);
}

#[test]
fn wood_ignites_past_its_threshold() {
    let m = mats();
    let ignite = m.get(id(&m, "wood")).catches_fire_at.unwrap();

    // Just below the threshold: no fire.
    let (mut w, i) = small(&m, "air", "wood");
    w.temp[i] = ignite - 60.0;
    w.step(&m);
    assert!(!w.is_burning(i));

    // Above it, touching air: burns.
    let (mut w, i) = small(&m, "air", "wood");
    w.temp[i] = ignite + 100.0;
    w.step(&m);
    assert!(w.is_burning(i), "wood at {}°C should burn", w.temp[i]);
}

#[test]
fn wood_needs_air_to_burn() {
    let m = mats();
    let (mut w, i) = small(&m, "stone", "wood");
    w.temp[i] = 900.0;
    w.step(&m);
    assert!(!w.is_burning(i), "wood sealed in stone shouldn't burn");
}

#[test]
fn burning_wood_spreads_and_leaves_a_little_ash() {
    let m = mats();
    // Tall enough that smoke doesn't fill the room and choke the fire.
    let mut w = World::new(30, 80);
    w.fill(&m, AIR);
    for x in 0..30 {
        w.set(&m, x, 79, id(&m, "stone"));
        w.set(&m, x, 78, id(&m, "wood"));
    }
    let start = w.idx(0, 78);
    w.temp[start] = 600.0;
    for _ in 0..2000 {
        w.step(&m);
    }
    let census = w.census(&m);
    let ash = census.iter().find(|c| c.0 == "ash").map(|c| c.1).unwrap_or(0);
    let wood = census.iter().find(|c| c.0 == "wood").map(|c| c.1).unwrap_or(0);
    assert!(wood <= 5, "fire should spread along the whole plank: {census:?}");
    assert!(ash >= 1 && ash <= 15, "only a little ash should be left from 30 wood: {census:?}");
}

#[test]
fn water_freezes_into_ice() {
    let m = mats();
    let (mut w, i) = small(&m, "stone", "water");
    w.temp[i] = -20.0;
    w.step(&m);
    assert_eq!(w.mat[i], id(&m, "ice"));
}

#[test]
fn water_boils_into_steam() {
    let m = mats();
    let (mut w, i) = small(&m, "stone", "water");
    w.temp[i] = 150.0;
    w.step(&m);
    assert_eq!(w.mat[i], id(&m, "steam"));
}

#[test]
fn ice_melts_in_warm_water() {
    let m = mats();
    let (mut w, i) = small(&m, "water", "ice");
    for _ in 0..300 {
        w.step(&m);
    }
    assert_ne!(w.mat[i], id(&m, "ice"));
}

#[test]
fn lava_and_water_make_stone_and_steam() {
    let m = mats();
    let mut w = World::new(20, 20);
    w.fill(&m, id(&m, "stone"));
    for x in 1..19 {
        for y in 1..10 {
            w.set(&m, x, y, id(&m, "water"));
        }
        for y in 10..19 {
            w.set(&m, x, y, id(&m, "lava"));
        }
    }
    for _ in 0..60 {
        w.step(&m);
    }
    let census = w.census(&m);
    let steam = census.iter().find(|c| c.0 == "steam").map(|c| c.1).unwrap_or(0);
    assert!(steam > 0, "{census:?}");
    let stone = census.iter().find(|c| c.0 == "stone").map(|c| c.1).unwrap_or(0);
    assert!(stone > 20 * 20 - 18 * 18, "new stone should form: {census:?}");
}

#[test]
fn sand_falls_and_steam_rises() {
    let m = mats();
    let mut w = World::new(10, 20);
    w.fill(&m, AIR);
    w.set(&m, 5, 2, id(&m, "sand"));
    w.set(&m, 2, 17, id(&m, "steam"));
    for _ in 0..12 {
        w.step(&m);
    }
    assert!(w.mat[w.idx(5, 2)] != id(&m, "sand"), "sand should fall");
    let steam_y = (0..w.mat.len()).find(|&i| w.mat[i] == id(&m, "steam")).map(|i| i / w.w);
    assert!(steam_y.is_some_and(|y| y < 12), "steam should rise, at {steam_y:?}");
}

#[test]
fn oil_floats_on_water() {
    let m = mats();
    let mut w = World::new(4, 10);
    w.fill(&m, AIR);
    for x in 0..4 {
        for y in 2..5 {
            w.set(&m, x, y, id(&m, "water"));
        }
        for y in 5..8 {
            w.set(&m, x, y, id(&m, "oil"));
        }
    }
    for _ in 0..200 {
        w.step(&m);
    }
    assert_eq!(w.mat[w.idx(1, 9)], id(&m, "water"));
    assert_eq!(w.mat[w.idx(1, 4)], id(&m, "oil"));
}

#[test]
fn scenes_round_trip() {
    let m = mats();
    let text = "size 32 16\nfill air\nrect stone 0 12 32 4\ncircle water 10 6 3\nline wood 20 2 30 10 1\n";
    let l = scene::load(text, &m, None);
    let (w, problems) = (l.world, l.problems);
    assert!(problems.is_empty(), "{problems:?}");
    let saved = scene::save(&grave_engine::sim::Snapshot::of_world(w.clone()), &m);
    let l2 = scene::load(&saved, &m, None);
    let (w2, problems) = (l2.world, l2.problems);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(w.mat, w2.mat);
}

#[test]
fn scene_problems_are_reported() {
    let m = mats();
    let problems = scene::load("size 32 16\nrect stoen 0 0 4 4\nwobble\n", &m, None).problems;
    assert_eq!(problems.len(), 2, "{problems:?}");
}

#[test]
fn bundled_scenes_load_cleanly() {
    let m = mats();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/scenes");
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "txt") {
            let problems = scene::load(&std::fs::read_to_string(&p).unwrap(), &m, p.parent()).problems;
            assert!(problems.is_empty(), "{}: {problems:?}", p.display());
        }
    }
}

#[test]
fn saving_with_lots_of_materials_round_trips() {
    // More materials than letters and digits, so odd legend characters get used.
    let names: Vec<String> = (0..80).map(|i| format!("m{i:02}")).collect();
    let texts: Vec<(String, String)> = names.iter().map(|n| (n.clone(), "state = solid\n".to_string())).collect();
    let refs: Vec<(&str, &str)> = texts.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let m = Materials::from_texts(&refs);
    let mut w = World::new(80, 8);
    for (x, n) in names.iter().enumerate() {
        w.set(&m, x, 0, m.id(n).unwrap());
    }
    let saved = scene::save(&grave_engine::sim::Snapshot::of_world(w.clone()), &m);
    let l = scene::load(&saved, &m, None);
    assert!(l.problems.is_empty(), "{:?}", l.problems);
    assert_eq!(l.world.mat, w.mat);
}

/// Average x position and horizontal spread of the smoke.
fn smoke_shape(w: &World, m: &Materials) -> (f32, f32) {
    let id = m.id("smoke").unwrap();
    let xs: Vec<f32> = (0..w.mat.len()).filter(|&i| w.mat[i] == id).map(|i| (i % w.w) as f32).collect();
    let n = xs.len().max(1) as f32;
    let mean = xs.iter().sum::<f32>() / n;
    let spread = (xs.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n).sqrt();
    (mean, spread)
}

fn smoky(m: &Materials, settings: &str) -> World {
    // A steady source of hot smoke in the middle of an open space.
    let text = format!("size 200 120\n{settings}\nfill air\nrect stone 0 110 200 10\n");
    let mut w = scene::load(&text, m, None).world;
    let smoke = m.id("smoke").unwrap();
    for f in 0..240 {
        if f < 120 {
            for x in 96..104 {
                w.set(m, x, 105, smoke);
                let i = w.idx(x, 105);
                w.temp[i] = 400.0;
            }
        }
        w.step(m);
    }
    w
}

#[test]
fn wind_blows_smoke_downwind() {
    let m = mats();
    let (calm, _) = smoke_shape(&smoky(&m, "wind 0"), &m);
    let (windy, _) = smoke_shape(&smoky(&m, "wind 4"), &m);
    assert!(windy > calm + 20.0, "smoke should drift right: calm at x={calm}, windy at x={windy}");
}

#[test]
fn smoke_billows_out_instead_of_a_straight_column() {
    let m = mats();
    let (_, still) = smoke_shape(&smoky(&m, "turbulence 0"), &m);
    let (_, swirly) = smoke_shape(&smoky(&m, "turbulence 1"), &m);
    assert!(swirly > still * 1.3, "swirling air should spread the smoke out: still {still}, swirly {swirly}");
}
