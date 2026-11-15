//! Level generator: herringbone Wang tiles (Sean Barrett's method, used by
//! Noita): https://nothings.org/gamedev/herringbone/
//!
//! The area is covered with bricks of 2x1 ("horizontal") and 1x2
//! ("vertical") tile units in a herringbone pattern: diagonal staircases of
//! one horizontal brick with a vertical one under its left half, repeated
//! every 4 units across. Bricks never line up in long rows, so there are no
//! grid seams.
//!
//! Every brick has six edge pieces (each long side counts as two). Each edge
//! piece two bricks share is randomly *open* (a passage crosses it, at its
//! middle) or *wall*, decided once for that piece, so passages always line
//! up from one brick to the next (the Wang-tile rule). Then each brick is
//! filled with a tile whose edges match:
//!
//! - **Hand-made tiles** (a folder of small text drawings, see `Tile::parse`)
//!   are used wherever one's edges fit, chosen at random among those that do.
//! - Otherwise a **built-in cave tile** is made on the spot: wobbly tunnels
//!   from each opening to a cavern in the brick's middle (each half of the
//!   brick is a square; both halves are joined), now and then a pool.
//!
//! The same seed always gives the same level. Scene line:
//! `generate X Y W H [tile 40] [seed 7] [rock rock] [open 0.55] [tiles DIR] [pools 0.3]`.

use crate::material::{AIR, MatId, Materials};
use crate::world::World;
use std::path::Path;

/// One brick's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// Two units wide, one tall.
    Horizontal,
    /// One unit wide, two tall.
    Vertical,
}

/// The six edge pieces of a brick, clockwise from the top left:
/// horizontal brick: top-left, top-right, right, bottom-right, bottom-left, left;
/// vertical brick: top, right-upper, right-lower, bottom, left-lower, left-upper.
pub type Edges = [bool; 6];

/// A hand-made tile: its shape, which edge pieces have a passage, and its
/// drawing (any size: it's stretched to the brick).
#[derive(Clone, Debug)]
pub struct Tile {
    pub name: String,
    pub shape: Shape,
    pub edges: Edges,
    pub w: usize,
    pub h: usize,
    /// Row by row; `None` = leave the fill (air).
    pub cells: Vec<Option<MatId>>,
}

impl Tile {
    /// A tile file:
    /// ```text
    /// shape h                 # h = 2 wide x 1 tall, v = 1 wide x 2 tall
    /// edges 1 0 1 1 0 1       # passages, clockwise from the top left (see `Edges`)
    /// legend # rock           # characters in the drawing (`.` = empty)
    /// legend ~ water
    /// rows
    /// ##########....##########
    /// ...
    /// end
    /// ```
    pub fn parse(name: &str, text: &str, mats: &Materials) -> Result<Tile, String> {
        let mut shape = None;
        let mut edges = None;
        let mut legend: Vec<(char, MatId)> = vec![('.', AIR)];
        let mut rows: Vec<Vec<char>> = Vec::new();
        let mut in_rows = false;
        for line in text.lines() {
            if in_rows {
                if line.trim() == "end" {
                    in_rows = false;
                    continue;
                }
                rows.push(line.trim_end().chars().collect());
                continue;
            }
            // `legend # rock`: the character may be `#`, so read it before
            // comments (`# ...`) are cut off.
            let raw = line.trim();
            let line = if raw.starts_with("legend") { raw } else { raw.split('#').next().unwrap_or("").trim() };
            let full = raw;
            let words: Vec<&str> = line.split_whitespace().collect();
            match words.first().copied() {
                None => {}
                Some("shape") => {
                    shape = Some(match words.get(1).copied() {
                        Some("h") | Some("horizontal") => Shape::Horizontal,
                        Some("v") | Some("vertical") => Shape::Vertical,
                        _ => return Err(format!("{name}: `shape h` or `shape v`")),
                    })
                }
                Some("edges") => {
                    let e: Vec<bool> = words[1..].iter().map(|w| *w == "1" || *w == "open").collect();
                    if e.len() != 6 {
                        return Err(format!("{name}: `edges` needs six 0/1 values"));
                    }
                    edges = Some([e[0], e[1], e[2], e[3], e[4], e[5]]);
                }
                Some("legend") => {
                    // `legend # rock`: the character may be `#`, so read the raw line.
                    let raw: Vec<&str> = full.split_whitespace().collect();
                    let (Some(c), Some(m)) = (raw.get(1).and_then(|c| c.chars().next()), raw.get(2)) else {
                        return Err(format!("{name}: `legend C material`"));
                    };
                    let id = mats.id(m).ok_or_else(|| format!("{name}: no material called `{m}`"))?;
                    legend.push((c, id));
                }
                Some("rows") => in_rows = true,
                Some(other) => return Err(format!("{name}: unknown line `{other}`")),
            }
        }
        let shape = shape.ok_or_else(|| format!("{name}: missing `shape`"))?;
        let edges = edges.ok_or_else(|| format!("{name}: missing `edges`"))?;
        let h = rows.len();
        let w = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        if w == 0 || h == 0 {
            return Err(format!("{name}: the drawing is empty"));
        }
        let mut cells = Vec::with_capacity(w * h);
        for r in &rows {
            for x in 0..w {
                let c = r.get(x).copied().unwrap_or('.');
                let id = legend.iter().rev().find(|e| e.0 == c).map(|e| e.1).ok_or_else(|| format!("{name}: `{c}` isn't in the legend"))?;
                cells.push((id != AIR).then_some(id));
            }
        }
        Ok(Tile { name: name.into(), shape, edges, w, h, cells })
    }

    /// Every `*.txt` tile in a folder (problems are reported, not fatal).
    pub fn load_dir(dir: &Path, mats: &Materials) -> (Vec<Tile>, Vec<String>) {
        let (mut tiles, mut problems) = (Vec::new(), Vec::new());
        let Ok(rd) = std::fs::read_dir(dir) else {
            problems.push(format!("no tile folder at {}", dir.display()));
            return (tiles, problems);
        };
        let mut paths: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "txt")).collect();
        paths.sort();
        for p in paths {
            let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| Tile::parse(&name, &t, mats)) {
                Ok(t) => tiles.push(t),
                Err(e) => problems.push(e),
            }
        }
        (tiles, problems)
    }
}

/// How to generate.
#[derive(Clone, Debug)]
pub struct Settings {
    /// Tile unit in cells (a horizontal brick is 2 units wide).
    pub tile: usize,
    pub seed: u32,
    /// What the ground is made of.
    pub rock: MatId,
    /// How likely each edge piece is a passage (0..1).
    pub open: f32,
    /// How often a built-in tile gets a pool of water in its cavern (0..1).
    pub pools: f32,
    /// The liquid for pools.
    pub pool: MatId,
    /// Let passages run out of the area's sides (otherwise it's sealed).
    pub open_sides: bool,
    /// Pin the generated ground so it never collapses on its own (blasts,
    /// fire and digging still break it). Off = thin walls and overhangs
    /// fall as the engine's strength rules say (Noita's lesson: a level left
    /// to real physics ends up as a pile of rubble).
    pub anchored: bool,
}

/// One brick of the layout: shape and top-left corner in units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Brick {
    pub shape: Shape,
    pub u: i32,
    pub v: i32,
}

impl Brick {
    /// Width and height in units.
    pub fn size(self) -> (i32, i32) {
        match self.shape {
            Shape::Horizontal => (2, 1),
            Shape::Vertical => (1, 2),
        }
    }

    /// Its six edge pieces, in `Edges` order, as keys shared with the
    /// neighbour on the other side: (along a horizontal line?, unit x, unit y).
    pub fn pieces(self) -> [(bool, i32, i32); 6] {
        let (u, v) = (self.u, self.v);
        match self.shape {
            Shape::Horizontal => [(true, u, v), (true, u + 1, v), (false, u + 2, v), (true, u + 1, v + 1), (true, u, v + 1), (false, u, v)],
            Shape::Vertical => [(true, u, v), (false, u + 1, v), (false, u + 1, v + 1), (true, u, v + 2), (false, u, v + 1), (false, u, v)],
        }
    }
}

/// The herringbone bricks covering a box `uw` x `uh` units (from 0,0).
/// Staircases: horizontal at (k + 4m, k), vertical at (k + 4m, k + 1).
pub fn layout(uw: i32, uh: i32) -> Vec<Brick> {
    let mut out = Vec::new();
    for k in -3..=uh + 1 {
        let m0 = (-3 - k).div_euclid(4) - 1;
        let m1 = (uw + 1 - k).div_euclid(4) + 1;
        for m in m0..=m1 {
            let u = k + 4 * m;
            for b in [Brick { shape: Shape::Horizontal, u, v: k }, Brick { shape: Shape::Vertical, u, v: k + 1 }] {
                let (w, h) = b.size();
                if b.u < uw && b.v < uh && b.u + w > 0 && b.v + h > 0 {
                    out.push(b);
                }
            }
        }
    }
    out
}

/// A repeatable random number from a seed and a few values.
fn hash(seed: u32, a: i32, b: i32, c: i32) -> u32 {
    let mut x = (seed as u64) ^ ((a as u32 as u64) << 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u32 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ (c as u32 as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    x as u32
}
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// Is this shared edge piece a passage? (Decided once per piece, so both
/// bricks touching it agree.) Pieces on the area's border are walls unless
/// `open_sides`.
pub fn piece_open(s: &Settings, key: (bool, i32, i32), uw: i32, uh: i32) -> bool {
    let (along, x, y) = key;
    // On the area's edge, or outside it altogether.
    let border = if along { y <= 0 || y >= uh || x < 0 || x >= uw } else { x <= 0 || x >= uw || y < 0 || y >= uh };
    if border && !s.open_sides {
        return false;
    }
    unit(hash(s.seed, along as i32 + 7, x, y)) < s.open
}

/// Fill a box of the world with a generated level. Returns how many bricks
/// used hand-made tiles.
pub fn generate(world: &mut World, mats: &Materials, area: (usize, usize, usize, usize), s: &Settings, tiles: &[Tile]) -> usize {
    let t = s.tile.max(8);
    let (ax, ay, aw, ah) = area;
    let (aw, ah) = (aw.min(world.w.saturating_sub(ax)), ah.min(world.h.saturating_sub(ay)));
    if aw == 0 || ah == 0 {
        return 0;
    }
    let (uw, uh) = (aw.div_ceil(t) as i32, ah.div_ceil(t) as i32);
    // Solid ground first, then each brick carves its passages. (Cells of
    // objects and characters are left alone throughout.)
    let owned: Vec<bool> = (ay..ay + ah).flat_map(|y| (ax..ax + aw).map(move |x| (x, y))).map(|(x, y)| world.is_owned(world.idx(x, y))).collect();
    let free = |x: i32, y: i32| !owned[(y as usize - ay) * aw + (x as usize - ax)];
    for y in ay..ay + ah {
        for x in ax..ax + aw {
            if free(x as i32, y as i32) {
                world.set(mats, x, y, s.rock);
            }
        }
    }
    let mut hand = 0;
    for b in layout(uw, uh) {
        let edges: Edges = b.pieces().map(|k| piece_open(s, k, uw, uh));
        let fits: Vec<&Tile> = tiles.iter().filter(|tile| tile.shape == b.shape && tile.edges == edges).collect();
        let (bw, bh) = b.size();
        let (px, py) = (ax as i32 + b.u * t as i32, ay as i32 + b.v * t as i32);
        let (pw, ph) = (bw as usize * t, bh as usize * t);
        let inside = |x: i32, y: i32| x >= ax as i32 && y >= ay as i32 && x < (ax + aw) as i32 && y < (ay + ah) as i32 && free(x, y);
        if !fits.is_empty() {
            let tile = fits[hash(s.seed, b.u, b.v, 99) as usize % fits.len()];
            hand += 1;
            for cy in 0..ph {
                for cx in 0..pw {
                    let (x, y) = (px + cx as i32, py + cy as i32);
                    if !inside(x, y) {
                        continue;
                    }
                    let c = tile.cells[(cy * tile.h / ph) * tile.w + cx * tile.w / pw];
                    world.set(mats, x as usize, y as usize, c.unwrap_or(AIR));
                }
            }
        } else {
            carve(world, mats, b, edges, (px, py), t, s, &inside);
        }
    }
    // Sealed: a solid rim, so no cavern bulges out of the sides.
    if !s.open_sides {
        for y in ay..ay + ah {
            for x in ax..ax + aw {
                let rim = x < ax + 2 || y < ay + 2 || x + 2 >= ax + aw || y + 2 >= ay + ah;
                if rim && free(x as i32, y as i32) {
                    world.set(mats, x, y, s.rock);
                }
            }
        }
    }
    if s.anchored {
        for y in ay..ay + ah {
            for x in ax..ax + aw {
                let i = world.idx(x, y);
                if !world.is_owned(i) && mats.get(world.mat[i]).state == crate::material::State::Solid {
                    world.flags[i] |= crate::world::ANCHORED;
                }
            }
        }
    }
    hand
}

/// The built-in cave tile for brick `b` (top-left at `at`, in cells).
fn carve(world: &mut World, mats: &Materials, b: Brick, edges: Edges, at: (i32, i32), t: usize, s: &Settings, inside: &dyn Fn(i32, i32) -> bool) {
    let tf = t as f32;
    let seed = hash(s.seed, b.u, b.v, b.shape as i32 + 3);
    let rnd = |n: i32| unit(hash(seed, n, 17, 29));
    // The two squares of the brick (their centres, in cells), and which
    // square each edge piece belongs to.
    let (c0, c1, owner): ((f32, f32), (f32, f32), [usize; 6]) = match b.shape {
        Shape::Horizontal => ((0.5, 0.5), (1.5, 0.5), [0, 1, 1, 1, 0, 0]),
        Shape::Vertical => ((0.5, 0.5), (0.5, 1.5), [0, 0, 1, 1, 1, 0]),
    };
    // Where each edge piece's middle is (in units, within the brick).
    let mids: [(f32, f32); 6] = match b.shape {
        Shape::Horizontal => [(0.5, 0.0), (1.5, 0.0), (2.0, 0.5), (1.5, 1.0), (0.5, 1.0), (0.0, 0.5)],
        Shape::Vertical => [(0.5, 0.0), (1.0, 0.5), (1.0, 1.5), (0.5, 2.0), (0.0, 1.5), (0.0, 0.5)],
    };
    let cell = |p: (f32, f32)| (at.0 as f32 + p.0 * tf, at.1 as f32 + p.1 * tf);
    // Squares wander a little from dead centre, so caverns don't line up.
    let wobble = |c: (f32, f32), n: i32| (c.0 + (rnd(n) - 0.5) * 0.3, c.1 + (rnd(n + 1) - 0.5) * 0.3);
    let centres = [cell(wobble(c0, 1)), cell(wobble(c1, 3))];
    let mut dig = |x: f32, y: f32, r: f32| {
        let ri = r.ceil() as i32;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let (cx, cy) = (x as i32 + dx, y as i32 + dy);
                if (dx * dx + dy * dy) as f32 <= r * r && inside(cx, cy) {
                    world.set(mats, cx as usize, cy as usize, AIR);
                }
            }
        }
    };
    // A wobbly tunnel. Ends are always the same width (`end_r`), so a
    // passage meets its neighbour's exactly at the shared edge.
    let end_r = (tf * 0.13).max(2.5);
    let tunnel = |a: (f32, f32), z: (f32, f32), n: i32, dig: &mut dyn FnMut(f32, f32, f32)| {
        let (dx, dy) = (z.0 - a.0, z.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let (nx, ny) = (-dy / len, dx / len);
        let steps = len.ceil() as i32;
        let (f1, f2) = (rnd(n) * 6.28, rnd(n + 1) * 6.28);
        for k in 0..=steps {
            let p = k as f32 / steps as f32;
            // Wobble most in the middle, none at the ends.
            let bend = (p * std::f32::consts::PI).sin();
            let off = bend * tf * 0.12 * ((p * 5.0 + f1).sin() + 0.5 * (p * 11.0 + f2).sin());
            let r = end_r * (1.0 + 0.35 * bend * (p * 7.0 + f2).sin().abs());
            dig(a.0 + dx * p + nx * off, a.1 + dy * p + ny * off, r);
        }
    };
    let mut used = [false; 2];
    for (n, open) in edges.iter().enumerate() {
        if *open && inside(centres[owner[n]].0 as i32, centres[owner[n]].1 as i32) {
            let sq = owner[n];
            used[sq] = true;
            tunnel(cell(mids[n]), centres[sq], 10 + n as i32 * 3, &mut dig);
        }
    }
    // Join the two halves, so every way in leads everywhere in the brick.
    if used[0] && used[1] {
        tunnel(centres[0], centres[1], 40, &mut dig);
    }
    // Caverns in the used halves (and, rarely, a hidden pocket): a few
    // overlapping blobs, some small, some roomy.
    let mut caverns = [0.0f32; 2];
    for sq in 0..2 {
        let (x, y) = centres[sq];
        if !inside(x as i32, y as i32) {
            continue;
        }
        if used[sq] || rnd(50 + sq as i32) < 0.12 {
            let r = tf * (0.18 + 0.16 * rnd(60 + sq as i32).powf(0.7));
            caverns[sq] = r;
            dig(x, y, r);
            for n in 0..3 {
                let a = rnd(70 + n * 2 + sq as i32 * 9) * 6.28;
                let d = r * 0.55;
                dig(x + a.cos() * d, y + a.sin() * d * 0.5, r * (0.5 + 0.3 * rnd(71 + n * 2)));
            }
        }
    }
    // Now and then a pool settles in the bottom of a cavern.
    if s.pools > 0.0 && rnd(80) < s.pools {
        let sq = if caverns[1] > 0.0 && rnd(81) < 0.5 { 1 } else { 0 };
        if caverns[sq] > 0.0 {
            let (x, y) = centres[sq];
            let r = caverns[sq] * 1.3;
            for cy in (y + caverns[sq] * 0.3) as i32..(y + r) as i32 {
                for cx in (x - r) as i32..(x + r) as i32 {
                    if inside(cx, cy) && world.mat[world.idx(cx as usize, cy as usize)] == AIR {
                        world.set(mats, cx as usize, cy as usize, s.pool);
                    }
                }
            }
        }
    }
}
