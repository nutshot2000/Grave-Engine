//! Level generator: herringbone Wang tiles (src/levelgen.rs).
use grave_engine::levelgen::{self, Settings, Shape, Tile, layout};
use grave_engine::{Materials, Sim, scene};
use std::collections::HashMap;

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn sim(m: &Materials, text: &str) -> Sim {
    let (s, problems) = scene::load(text, m, Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))).into_sim(m);
    assert!(problems.is_empty(), "{problems:?}");
    s
}

#[test]
fn the_bricks_cover_the_area_exactly_once() {
    let (uw, uh) = (23, 14);
    let mut covered = vec![0u8; (uw * uh) as usize];
    for b in layout(uw, uh) {
        let (w, h) = b.size();
        for v in b.v..b.v + h {
            for u in b.u..b.u + w {
                if u >= 0 && v >= 0 && u < uw && v < uh {
                    covered[(v * uw + u) as usize] += 1;
                }
            }
        }
    }
    assert!(covered.iter().all(|c| *c == 1), "every unit covered once: {covered:?}");
}

#[test]
fn every_shared_edge_belongs_to_two_bricks() {
    // (So both sides of an edge agree on whether a passage crosses it.)
    let (uw, uh) = (20, 12);
    let mut count: HashMap<(bool, i32, i32), u32> = HashMap::new();
    for b in layout(uw, uh) {
        for k in b.pieces() {
            *count.entry(k).or_default() += 1;
        }
    }
    for (&(along, x, y), &n) in &count {
        let inner = if along { y > 0 && y < uh && x >= 0 && x < uw } else { x > 0 && x < uw && y >= 0 && y < uh };
        if inner {
            assert_eq!(n, 2, "edge piece {:?} is shared by {n} bricks", (along, x, y));
        }
    }
}

fn caves(m: &Materials, seed: u32) -> Sim {
    sim(m, &format!("size 640 320\nfill air\ngenerate 0 0 640 320 tile 40 seed {seed}\n"))
}

#[test]
fn the_same_seed_makes_the_same_caves() {
    let m = mats();
    let (a, b, c) = (caves(&m, 7), caves(&m, 7), caves(&m, 8));
    assert!(a.world.mat == b.world.mat, "same seed, same caves");
    assert!(a.world.mat != c.world.mat, "another seed, other caves");
}

#[test]
fn the_caves_hang_together() {
    let m = mats();
    let s = caves(&m, 3);
    let w = &s.world;
    let air: Vec<bool> = w.mat.iter().map(|&c| c == 0).collect();
    let total = air.iter().filter(|a| **a).count();
    assert!(total > w.mat.len() / 5, "there should be plenty of cave ({total} cells)");
    // The biggest connected cave.
    let mut seen = vec![false; air.len()];
    let mut biggest = 0;
    for start in 0..air.len() {
        if !air[start] || seen[start] {
            continue;
        }
        let (mut n, mut stack) = (0, vec![start]);
        seen[start] = true;
        while let Some(i) = stack.pop() {
            n += 1;
            let (x, y) = (i % w.w, i / w.w);
            for (ok, j) in [(x > 0, i.wrapping_sub(1)), (x + 1 < w.w, i + 1), (y > 0, i.wrapping_sub(w.w)), (y + 1 < w.h, i + w.w)] {
                if ok && air[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        biggest = biggest.max(n);
    }
    assert!(biggest * 10 > total * 6, "most of the cave should be one connected network: {biggest} of {total}");
    // Sealed: no passage runs out of the sides.
    for y in 0..w.h {
        for x in [0, w.w - 1] {
            assert_ne!(w.mat[w.idx(x, y)], 0, "open at the side ({x}, {y})");
        }
    }
}

#[test]
fn generated_caves_dont_collapse() {
    let m = mats();
    let mut s = caves(&m, 5);
    let rock = m.id("rock").unwrap();
    let before = s.world.mat.iter().filter(|&&c| c == rock).count();
    for _ in 0..120 {
        s.step(&m);
    }
    let after = s.world.mat.iter().filter(|&&c| c == rock).count();
    assert_eq!(before, after, "anchored rock stays put");
}

#[test]
fn hand_made_tiles_are_used_where_they_fit() {
    let m = mats();
    let (tiles, problems) = Tile::load_dir(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tiles/rooms"), &m);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(tiles.len(), 2);
    assert!(tiles.iter().any(|t| t.shape == Shape::Horizontal) && tiles.iter().any(|t| t.shape == Shape::Vertical));
    // Lots of bricks, so some will match the sample tiles' edges.
    let mut s = Sim::empty(1200, 600, &m);
    let set = Settings { tile: 40, seed: 11, open: 0.5, pools: 0.0, ..Settings::new(&m) };
    let used = levelgen::generate(&mut s.world, &m, (0, 0, 1200, 600), &set, &tiles);
    assert!(used > 0, "no brick used a hand-made tile");
    let brick = m.id("brick").unwrap();
    assert!(s.world.mat.contains(&brick), "the hall's brick floor should be in the level");
    // The scene line does the same.
    let s = sim(&m, "size 600 300\nfill air\ngenerate 0 0 600 300 tile 40 seed 11 tiles tiles/rooms pools 0\n");
    assert!(s.world.mat.iter().any(|&c| c != 0));
}

#[test]
fn tile_files_say_what_is_wrong() {
    let m = mats();
    assert!(Tile::parse("t", "shape h\nedges 1 0 1\nrows\n##\nend\n", &m).unwrap_err().contains("six"));
    assert!(Tile::parse("t", "shape q\n", &m).is_err());
    assert!(Tile::parse("t", "shape h\nedges 0 0 0 0 0 0\nrows\n#x\nend\n", &m).unwrap_err().contains("legend"));
    let (_, problems) = scene::load("size 100 100\ngenerate 0 0 100 100 wobble 3\n", &m, None).into_sim(&m);
    assert!(!problems.is_empty());
}

#[test]
fn the_wall_behind_the_caves_is_saved() {
    let m = mats();
    let mut s = sim(&m, "size 200 100\nfill air\ngenerate 0 0 200 100 seed 3\n");
    let rock = m.id("rock").unwrap();
    assert!(s.world.back.iter().all(|&b| b == rock), "a rock wall behind the whole area");
    let text = scene::save(&s.snapshot(), &m);
    assert!(text.contains("backwall rock 0 0 200 1"), "saved as runs");
    let again = sim(&m, &text);
    assert!(again.world.back == s.world.back);
    let none = sim(&m, "size 100 50\nfill air\ngenerate 0 0 100 50 seed 3 back none\n");
    assert!(none.world.back.iter().all(|&b| b == 0));
}
