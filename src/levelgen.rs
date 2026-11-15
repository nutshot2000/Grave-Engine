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
//!   are used wherever one's edges fit, chosen at random among those that do
//!   (or, if none fits exactly, one with a single spare doorway onto rock).
//! - Otherwise a **built-in cave tile** is made on the spot: wobbly tunnels
//!   from each opening to a cavern in the brick's middle (each half of the
//!   brick is a square; both halves are joined), now and then a pool.
//!
//! The same seed always gives the same level. Scene line:
//! `generate X Y W H [tile 40] [seed 7] [rock rock] [open 0.55] [tiles DIR] [pools 0.3]`.

use crate::material::{AIR, MatId, Materials};
use crate::world::World;
use std::path::Path;

/// In a tile's drawing: the level's own ground material (`legend # ground`).
pub const GROUND: MatId = MatId::MAX;

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
    /// Also use it flipped left to right (`mirror no` in the file turns it off).
    pub mirror: bool,
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
        let mut mirror = true;
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
                    // `ground` = whatever the level's rock is.
                    let id = if *m == "ground" { GROUND } else { mats.id(m).ok_or_else(|| format!("{name}: no material called `{m}`"))? };
                    legend.push((c, id));
                }
                Some("rows") => in_rows = true,
                Some("mirror") => mirror = !matches!(words.get(1).copied(), Some("no") | Some("off")),
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
        Ok(Tile { name: name.into(), shape, edges, w, h, cells, mirror })
    }

    /// The same tile flipped left to right (its edges swap sides).
    pub fn mirrored(&self) -> Tile {
        let e = self.edges;
        let edges = match self.shape {
            // top-left, top-right, right, bottom-right, bottom-left, left
            Shape::Horizontal => [e[1], e[0], e[5], e[4], e[3], e[2]],
            // top, right-upper, right-lower, bottom, left-lower, left-upper
            Shape::Vertical => [e[0], e[5], e[4], e[3], e[2], e[1]],
        };
        let mut cells = self.cells.clone();
        for y in 0..self.h {
            cells[y * self.w..(y + 1) * self.w].reverse();
        }
        Tile { name: format!("{} (mirrored)", self.name), edges, cells, mirror: false, ..self.clone() }
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
                Ok(t) => {
                    if t.mirror {
                        tiles.push(t.mirrored());
                    }
                    tiles.push(t)
                }
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
    /// The wall drawn behind the caves (`AIR` = none: the sky shows).
    pub back: MatId,
    /// How often a tile gets a special room (vault, ore ring, flooded room,
    /// lava pit, powder cache), 0..1.
    pub rooms: f32,
    /// Chance of a large cavern in each map sector (spans several tiles).
    pub caverns: f32,
    /// How strongly natural plans favour blind side passages (0..1).
    pub dead_ends: f32,
    /// Frequency of crystal clusters and torches around large chambers.
    pub crystals: f32,
    pub torches: f32,
}

impl Settings {
    /// The usual settings: rock caves, tile 64, rock wall behind.
    pub fn new(mats: &Materials) -> Settings {
        let rock = mats.id("rock").unwrap_or(AIR);
        Settings {
            tile: 64,
            seed: 1,
            rock,
            open: 0.6,
            pools: 0.2,
            pool: mats.id("water").unwrap_or(AIR),
            open_sides: false,
            anchored: true,
            back: rock,
            rooms: 0.25,
            caverns: 0.65,
            dead_ends: 0.75,
            crystals: 0.7,
            torches: 0.3,
        }
    }
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
    layout_range((0, 0), (uw, uh))
}

/// The bricks touching units `from` (inclusive) to `to` (exclusive),
/// anywhere on the endless map: the pattern is the same everywhere.
pub fn layout_range(from: (i32, i32), to: (i32, i32)) -> Vec<Brick> {
    let mut out = Vec::new();
    for k in from.1 - 3..=to.1 + 1 {
        let m0 = (from.0 - 3 - k).div_euclid(4) - 1;
        let m1 = (to.0 + 1 - k).div_euclid(4) + 1;
        for m in m0..=m1 {
            let u = k + 4 * m;
            for b in [Brick { shape: Shape::Horizontal, u, v: k }, Brick { shape: Shape::Vertical, u, v: k + 1 }] {
                let (w, h) = b.size();
                if b.u < to.0 && b.v < to.1 && b.u + w > from.0 && b.v + h > from.1 {
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
    // (The area's corner is the map's 0,0.)
    generate_at(world, mats, area, (-(area.0 as i64), -(area.1 as i64)), s, tiles)
}

/// Like `generate`, for part of an endless map: world cell (x, y) is map
/// cell `origin + (x, y)`. Whatever part is generated, and in whatever
/// order, the map comes out the same (bricks, doorways, tiles, ore are all
/// decided by map position). Endless maps use `open_sides`.
pub fn generate_at(world: &mut World, mats: &Materials, area: (usize, usize, usize, usize), origin: (i64, i64), s: &Settings, tiles: &[Tile]) -> usize {
    let t = s.tile.max(8);
    let (ax, ay, aw, ah) = area;
    let (aw, ah) = (aw.min(world.w.saturating_sub(ax)), ah.min(world.h.saturating_sub(ay)));
    if aw == 0 || ah == 0 {
        return 0;
    }
    // The area on the map, and the units it covers. (A sealed area's edges
    // are units 0 and uw, uh: only for `generate`, whose corner is 0,0.)
    let (gx, gy) = (origin.0 + ax as i64, origin.1 + ay as i64);
    let ti = t as i64;
    let (uw, uh) = (aw.div_ceil(t) as i32, ah.div_ceil(t) as i32);
    let from = (gx.div_euclid(ti) as i32, gy.div_euclid(ti) as i32);
    let to = ((gx + aw as i64 - 1).div_euclid(ti) as i32 + 1, (gy + ah as i64 - 1).div_euclid(ti) as i32 + 1);
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
    let caverns = large_caverns(s, (gx, gy, aw, ah));
    for b in layout_range(from, to) {
        let edges: Edges = b.pieces().map(|k| piece_open(s, k, uw, uh));
        // Exact fits first; failing that, a tile with one spare doorway (it
        // opens onto solid rock: a small dead-end notch). A tile never blocks
        // a passage the brick needs.
        let extra = |tile: &Tile| (0..6).filter(|&k| tile.edges[k] && !edges[k]).count();
        let covers = |tile: &Tile| tile.shape == b.shape && (0..6).all(|k| !edges[k] || tile.edges[k]);
        let mut fits: Vec<&Tile> = tiles.iter().filter(|tile| covers(tile) && extra(tile) == 0).collect();
        if fits.is_empty() && edges.iter().any(|e| *e) {
            fits = tiles.iter().filter(|tile| covers(tile) && extra(tile) == 1).collect();
        }
        let (bw, bh) = b.size();
        let (px, py) = ((b.u as i64 * ti - origin.0) as i32, (b.v as i64 * ti - origin.1) as i32);
        let (pw, ph) = (bw as usize * t, bh as usize * t);
        let inside = |x: i32, y: i32| x >= ax as i32 && y >= ay as i32 && x < (ax + aw) as i32 && y < (ay + ah) as i32 && free(x, y);
        // Landmarks punctuate natural caves rather than repeating in every
        // matching brick. The rest get a fresh seeded layout.
        let natural = caverns.iter().any(|c| c.overlaps(b.u as f32 * t as f32, b.v as f32 * t as f32, pw as f32, ph as f32));
        if !natural && !fits.is_empty() && unit(hash(s.seed, b.u, b.v, 101)) < 0.38 {
            let tile = fits[hash(s.seed, b.u, b.v, 99) as usize % fits.len()];
            hand += 1;
            for cy in 0..ph {
                for cx in 0..pw {
                    let (x, y) = (px + cx as i32, py + cy as i32);
                    if !inside(x, y) {
                        continue;
                    }
                    let (sx, sy) = cave_sample(cx, cy, pw, ph, tile.w, tile.h, hash(s.seed, b.u, b.v, 102));
                    let original = tile.cells[(cy * tile.h / ph) * tile.w + cx * tile.w / pw];
                    let sampled = natural_sample(&tile.cells, tile.w, tile.h, sx, sy, None, Some(GROUND));
                    let c = keep_feature(original, sampled, None, Some(GROUND));
                    let m = match c {
                        Some(GROUND) => s.rock,
                        Some(m) => m,
                        None => AIR,
                    };
                    world.set(mats, x as usize, y as usize, m);
                }
            }
        } else {
            if natural {
                let seed = hash(s.seed, b.u, b.v, b.shape as i32 + 3);
                let (grid, gw, gh) = organic_grid(b.shape, edges, seed, s.pools, s.dead_ends);
                draw_cave_blocks(world, mats, &grid, (gw, gh, 16), (px, py), t, seed, s, &inside);
            } else {
                carve(world, mats, b, edges, (px, py), t, s, &inside);
            }
        }
    }
    carve_large_caverns(world, mats, (ax, ay, aw, ah), origin, s, &caverns, &free);
    // Varied ground: patches of other stone, and ore specks in the walls.
    texture_ground(world, mats, (ax, ay, aw, ah), origin, s, &free);
    // The wall behind the caves.
    for y in ay..ay + ah {
        for x in ax..ax + aw {
            let i = world.idx(x, y);
            world.back[i] = s.back;
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
    cave_fixtures(world, mats, (ax, ay, aw, ah), origin, s, &caverns, &free);
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

/// A cavern in map coordinates, independent of the current streaming window.
struct LargeCavern { x: f32, y: f32, rx: f32, ry: f32, seed: u32 }

impl LargeCavern {
    fn overlaps(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        x < self.x + self.rx * 1.35 && x + w > self.x - self.rx * 1.35
            && y < self.y + self.ry * 1.35 && y + h > self.y - self.ry * 1.35
    }

    /// The unsculpted chamber outline, evaluated in map coordinates.
    fn contains(&self, mx: f32, my: f32, tile: usize) -> bool {
        let (mut nx, mut ny) = ((mx - self.x) / self.rx, (my - self.y) / self.ry);
        let phase = unit(hash(self.seed, 0, 0, 915)) * std::f32::consts::TAU;
        nx += 0.14 * (ny * 5.0 + phase).sin();
        ny += 0.12 * (nx * 6.0 - phase).sin();
        let scale = tile.max(8) as f32 * 0.35;
        let edge = 1.0 + (noise(self.seed, mx / scale, my / scale) - 0.5) * 0.5;
        nx * nx + ny * ny < edge
    }
}

/// Rooted cavern formations: a tapered ceiling tooth, a floor pillar or a
/// wall shelf. Their map-space plans are shared by every streamed region.
struct CavernRock { x: f32, y: f32, width: f32, height: f32, kind: u32 }

impl CavernRock {
    fn contains(&self, x: f32, y: f32) -> bool {
        let (dx, dy) = (x - self.x, y - self.y);
        match self.kind {
            // Ceiling teeth taper down from their broad attachment.
            0 => dy >= 0.0 && dy <= self.height && dx.abs() < self.width * (1.0 - dy / self.height).max(0.05),
            // Floor pillars taper towards an uneven, blunt top.
            1 => dy <= 0.0 && dy >= -self.height && dx.abs() < self.width * (1.0 + 0.65 * dy / self.height),
            // Width is signed: shelves point inwards from either wall.
            _ => {
                let along = dx / self.width;
                (0.0..=1.0).contains(&along) && dy >= 0.0 && dy < self.height * (1.0 - 0.7 * along)
            }
        }
    }
}

/// Find the real chamber edge at each attachment, rather than leaving
/// floating formations inside an approximate ellipse. A clear band through
/// the middle keeps the sculptures from walling off the cavern's main route.
fn cavern_rocks(c: &LargeCavern, tile: usize) -> Vec<CavernRock> {
    let r = |k| unit(hash(c.seed, k, 0, 930));
    let mut rocks = Vec::new();
    let gap = (tile as f32 * 0.16).max(9.0);
    let teeth = 4 + (r(80) * 5.0) as i32;
    for i in 0..teeth {
        let x = c.x + (-0.75 + i as f32 / (teeth - 1) as f32 * 1.5 + (r(i * 5) - 0.5) * 0.15) * c.rx;
        let ys: Vec<_> = ((c.y - c.ry * 1.35).floor() as i32..=(c.y + c.ry * 1.35).ceil() as i32)
            .filter(|&y| c.contains(x, y as f32, tile)).collect();
        let (Some(&roof), Some(&floor)) = (ys.first(), ys.last()) else { continue; };
        let length = (c.ry * (0.25 + r(i * 5 + 1) * 0.55)).min(c.y - gap - roof as f32);
        if length > 3.0 {
            rocks.push(CavernRock { x, y: roof as f32 - 8.0, width: c.rx * (0.025 + r(i * 5 + 2) * 0.035), height: length + 8.0, kind: 0 });
        }
        if i % 2 == 1 && r(i * 5 + 3) > 0.2 {
            let height = (c.ry * (0.45 + r(i * 5 + 3) * 0.75)).min(floor as f32 - c.y - gap);
            if height > 3.0 {
                rocks.push(CavernRock { x, y: floor as f32 + 8.0, width: c.rx * (0.035 + r(i * 5 + 4) * 0.05), height: height + 8.0, kind: 1 });
            }
        }
    }
    // Two or three ledges on each wall, above/below the open middle route.
    for side in [-1.0, 1.0] {
        for i in 0..3 {
            let y = c.y + [-0.5, 0.3, 0.65][i] * c.ry;
            let xs: Vec<_> = ((c.x - c.rx * 1.35).floor() as i32..=(c.x + c.rx * 1.35).ceil() as i32)
                .filter(|&x| c.contains(x as f32, y, tile)).collect();
            let edge = if side < 0.0 { xs.first() } else { xs.last() };
            if let Some(&x) = edge {
                rocks.push(CavernRock { x: x as f32 + side * 8.0, y, width: -side * (c.rx * (0.25 + r(60 + i as i32) * 0.3) + 8.0), height: (tile as f32 * 0.09).max(4.0), kind: 2 });
            }
        }
    }
    rocks
}

/// Sparse large-scale rooms break the tile-size ceiling. Include a whole
/// brick's padding so partial-region generation makes the same tile choices.
fn large_caverns(s: &Settings, (x, y, w, h): (i64, i64, usize, usize)) -> Vec<LargeCavern> {
    let t = s.tile.max(8) as i64;
    let (sw, sh) = (8 * t, 6 * t);
    let mut result = Vec::new();
    for sy in (y - 6 * t).div_euclid(sh)..=(y + h as i64 + 6 * t).div_euclid(sh) {
        for sx in (x - 6 * t).div_euclid(sw)..=(x + w as i64 + 6 * t).div_euclid(sw) {
            let seed = hash(s.seed, sx as i32, sy as i32, 910);
            if unit(seed) >= s.caverns { continue; }
            let r = |k| unit(hash(seed, k, 11, 73));
            let c = LargeCavern {
                x: (sx * sw) as f32 + (0.3 + r(1) * 0.4) * sw as f32,
                y: (sy * sh) as f32 + (0.3 + r(2) * 0.4) * sh as f32,
                rx: t as f32 * (1.7 + r(3) * 1.1),
                ry: t as f32 * (0.9 + r(4) * 0.65), seed,
            };
            if c.overlaps((x - 2 * t) as f32, (y - 2 * t) as f32, (w as i64 + 4 * t) as f32, (h as i64 + 4 * t) as f32) { result.push(c); }
        }
    }
    result
}

/// Open huge, slightly uneven chambers across brick boundaries. Owned cells
/// remain untouched; natural-only bricks around each chamber avoid cut-off
/// pieces of authored masonry. The final sealed-world rim is applied later.
fn carve_large_caverns(world: &mut World, mats: &Materials, (ax, ay, aw, ah): (usize, usize, usize, usize), origin: (i64, i64), s: &Settings, caverns: &[LargeCavern], free: &dyn Fn(i32, i32) -> bool) {
    for c in caverns {
        let rocks = cavern_rocks(c, s.tile);
        let x0 = ((c.x - c.rx * 1.35 - origin.0 as f32).floor() as i64).max(ax as i64) as usize;
        let y0 = ((c.y - c.ry * 1.35 - origin.1 as f32).floor() as i64).max(ay as i64) as usize;
        let x1 = ((c.x + c.rx * 1.35 - origin.0 as f32).ceil() as i64).clamp(0, (ax + aw) as i64) as usize;
        let y1 = ((c.y + c.ry * 1.35 - origin.1 as f32).ceil() as i64).clamp(0, (ay + ah) as i64) as usize;
        for y in y0..y1 {
            for x in x0..x1 {
                if !free(x as i32, y as i32) { continue; }
                let (mx, my) = ((origin.0 + x as i64) as f32, (origin.1 + y as i64) as f32);
                if c.contains(mx, my, s.tile) {
                    let rock = rocks.iter().any(|r| r.contains(mx, my));
                    world.set(mats, x, y, if rock { s.rock } else { AIR });
                }
            }
        }
    }
}

/// Seeded editable material fixtures. Plans come from the full map-space
/// chamber geometry, so a cluster split between streamed regions still joins.
fn cave_fixtures(world: &mut World, mats: &Materials, area: (usize, usize, usize, usize), origin: (i64, i64), s: &Settings, caverns: &[LargeCavern], free: &dyn Fn(i32, i32) -> bool) {
    let (ax, ay, aw, ah) = area;
    let cyan = mats.id("crystal_cyan");
    let violet = mats.id("crystal_violet");
    let flame = mats.id("torch_glow");
    let wood = mats.id("wood").unwrap_or(s.rock);
    for c in caverns {
        let rocks = cavern_rocks(c, s.tile);
        let open = |x: i32, y: i32| c.contains(x as f32, y as f32, s.tile) && !rocks.iter().any(|r| r.contains(x as f32, y as f32));
        for site in 0..7 {
            let r = |k| unit(hash(c.seed, site, k, 960));
            let torch = site == 3 || site == 6;
            if r(0) > if torch { s.torches } else { s.crystals } { continue; }
            let Some(mat) = (if torch { flame } else if r(1) < 0.55 { cyan } else { violet }) else { continue; };
            let x = (c.x + (-0.65 + site as f32 / 6.0 * 1.3 + (r(2) - 0.5) * 0.1) * c.rx).round() as i32;
            let hanging = !torch && site % 3 == 1;
            let direction = if hanging { -1 } else { 1 };
            let mut base = c.y.round() as i32;
            if !open(x, base) { continue; }
            for _ in 0..(c.ry * 1.5) as usize {
                if !open(x, base + direction) { break; }
                base += direction;
            }
            if (base as f32 - c.y).abs() < 24.0 { continue; }
            let mut put = |mx: i32, my: i32, id: MatId, color: Option<[u8; 3]>| {
                let (wx, wy) = (mx as i64 - origin.0, my as i64 - origin.1);
                if wx < ax as i64 || wy < ay as i64 || wx >= (ax + aw) as i64 || wy >= (ay + ah) as i64 { return; }
                if !s.open_sides && (wx < ax as i64 + 2 || wy < ay as i64 + 2 || wx + 2 >= (ax + aw) as i64 || wy + 2 >= (ay + ah) as i64) { return; }
                if !free(wx as i32, wy as i32) || !open(mx, my) { return; }
                world.set(mats, wx as usize, wy as usize, id);
                if let Some(color) = color { let i = world.idx(wx as usize, wy as usize); world.paint[i] = crate::asset::pack_paint(color); }
            };
            if torch {
                for dy in 0..8 { for dx in -1..=0 { put(x + dx, base - dy, wood, None); } }
                for dy in 0..6 { for dx in -2i32..=2 {
                    if dx.abs() <= (5 - dy) / 2 { put(x + dx, base - 8 - dy, mat, Some(if dx == 0 { [255, 207, 90] } else { [255, 126, 40] })); }
                } }
            } else {
                let color = mats.get(mat).color;
                for spike in -1i32..=1 {
                    let height = 6 + (r(4 + spike) * 7.0) as i32;
                    for dy in 0..height { for dx in -2i32..=2 {
                        if dx.abs() <= ((height - dy) as f32 / height as f32 * 2.5) as i32 {
                            let shade = if dx <= 0 { 1.1 } else { 0.68 };
                            put(x + spike * 3 + dx, base - direction * dy, mat, Some(color.map(|v| (v as f32 * shade).min(255.0) as u8)));
                        }
                    } }
                }
            }
        }
    }
}

/// Blocks across one tile unit in the built-in tiles (each block is
/// `tile / GRID` cells): the coarse grid the tiles are drawn on, like the
/// pixels of Sean Barrett's hand-drawn tiles.
const GRID: i32 = 8;

/// What a block of a built-in tile is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
    Ground,
    Open,
    Brick,
    Stone,
    Water,
    Lava,
    Powder,
    Wood,
    Ore,
}

/// A material by name, or a fallback.
fn pick(mats: &Materials, name: &str, or: MatId) -> MatId {
    mats.id(name).unwrap_or(or)
}

/// The built-in tile for brick `b` (top-left at `at`, in cells), drawn on a
/// coarse grid: corridors (straight, turning, or stepping diagonally) from
/// each opening to a hub in its half, rooms at the hubs, dead-end branches,
/// and now and then a special room.
fn carve(world: &mut World, mats: &Materials, b: Brick, edges: Edges, at: (i32, i32), t: usize, s: &Settings, inside: &dyn Fn(i32, i32) -> bool) {
    let seed = hash(s.seed, b.u, b.v, b.shape as i32 + 3);
    // Keep occasional built rooms among much more varied natural layouts.
    if hash(seed, 0, 0, 811) % 5 != 0 {
        let (grid, gw, gh) = organic_grid(b.shape, edges, seed, s.pools, s.dead_ends);
        draw_cave_blocks(world, mats, &grid, (gw, gh, 16), at, t, seed, s, inside);
        return;
    }
    let g = GRID;
    let (gw, gh) = match b.shape {
        Shape::Horizontal => (2 * g, g),
        Shape::Vertical => (g, 2 * g),
    };
    let seed = hash(s.seed, b.u, b.v, b.shape as i32 + 3);
    let mut n = 0i32;
    let mut rnd = || {
        n += 1;
        unit(hash(seed, n, 17, 29))
    };
    let mut grid = vec![Block::Ground; (gw * gh) as usize];
    let set = |grid: &mut Vec<Block>, x: i32, y: i32, v: Block| {
        if x >= 0 && y >= 0 && x < gw && y < gh {
            grid[(y * gw + x) as usize] = v;
        }
    };
    // The two halves (squares) of the brick, in blocks.
    let halves: [(i32, i32); 2] = match b.shape {
        Shape::Horizontal => [(0, 0), (g, 0)],
        Shape::Vertical => [(0, 0), (0, g)],
    };
    // Each edge piece: where it starts (blocks), which way it runs, which
    // way is inwards, and which half it belongs to (same order as `Edges`).
    let pieces: [((i32, i32), (i32, i32), (i32, i32), usize); 6] = match b.shape {
        Shape::Horizontal => [
            ((0, 0), (1, 0), (0, 1), 0),
            ((g, 0), (1, 0), (0, 1), 1),
            ((2 * g - 1, 0), (0, 1), (-1, 0), 1),
            ((g, g - 1), (1, 0), (0, -1), 1),
            ((0, g - 1), (1, 0), (0, -1), 0),
            ((0, 0), (0, 1), (1, 0), 0),
        ],
        Shape::Vertical => [
            ((0, 0), (1, 0), (0, 1), 0),
            ((g - 1, 0), (0, 1), (-1, 0), 0),
            ((g - 1, g), (0, 1), (-1, 0), 1),
            ((0, 2 * g - 1), (1, 0), (0, -1), 1),
            ((0, g), (0, 1), (1, 0), 1),
            ((0, 0), (0, 1), (1, 0), 0),
        ],
    };
    // Hubs: a spot in each half, away from its edges.
    let hubs: Vec<(i32, i32)> = halves.iter().map(|&(hx, hy)| (hx + 2 + (rnd() * (g - 4) as f32) as i32, hy + 2 + (rnd() * (g - 4) as f32) as i32)).collect();
    let mut used = [false; 2];
    for (k, open) in edges.iter().enumerate() {
        if *open {
            used[pieces[k].3] = true;
        }
    }
    // Rooms at the hubs, and maybe a special room in one of them.
    let mut rooms: [Option<(i32, i32, i32, i32)>; 2] = [None, None];
    for h in 0..2 {
        if used[h] && rnd() < 0.55 {
            let (rw, rh) = (2 + (rnd() * 5.0) as i32, 2 + (rnd() * 4.0) as i32);
            let (hx, hy) = halves[h];
            let x0 = (hubs[h].0 - rw / 2).clamp(hx + 1, hx + g - 1 - rw);
            let y0 = (hubs[h].1 - rh / 2).clamp(hy + 1, hy + g - 1 - rh);
            rooms[h] = Some((x0, y0, rw, rh));
        }
    }
    let special = if rnd() < s.rooms { Some((rnd() * 5.0) as u32) } else { None };
    let special_half = if used[1] && (!used[0] || rnd() < 0.5) { 1 } else { 0 };
    if let Some(kind) = special
        && used[special_half]
    {
        // A special room is a bit bigger, filling most of its half.
        let (hx, hy) = halves[special_half];
        let (rw, rh) = (4 + (rnd() * 2.0) as i32, 3 + (rnd() * 2.0) as i32);
        let (x0, y0) = (hx + (g - rw) / 2, hy + (g - rh) / 2);
        rooms[special_half] = Some((x0, y0, rw, rh));
        let wall = match kind {
            0 => Some(Block::Brick),
            1 => Some(Block::Stone),
            _ => None,
        };
        if let Some(wall) = wall {
            for y in y0 - 1..=y0 + rh {
                for x in x0 - 1..=x0 + rw {
                    set(&mut grid, x, y, wall);
                }
            }
        }
    }
    for room in rooms.iter().flatten() {
        let (x0, y0, rw, rh) = *room;
        for y in y0..y0 + rh {
            for x in x0..x0 + rw {
                set(&mut grid, x, y, Block::Open);
            }
        }
    }
    // A corridor from (x0,y0) to (x1,y1), `wide` blocks across: an L (either
    // way round) or a staircase of small steps.
    let corridor = |grid: &mut Vec<Block>, a: (i32, i32), z: (i32, i32), wide: i32, style: f32| {
        let mut dig = |x: i32, y: i32| {
            for dy in 0..wide {
                for dx in 0..wide {
                    let (cx, cy) = (x + dx - wide / 2, y + dy - wide / 2);
                    if cx >= 0 && cy >= 0 && cx < gw && cy < gh {
                        grid[(cy * gw + cx) as usize] = Block::Open;
                    }
                }
            }
        };
        let (mut x, mut y) = a;
        dig(x, y);
        if style < 0.4 {
            // Staircase: one step across, one step along, until lined up.
            while (x, y) != z {
                if x != z.0 {
                    x += (z.0 - x).signum();
                    dig(x, y);
                }
                if y != z.1 {
                    y += (z.1 - y).signum();
                    dig(x, y);
                }
            }
        } else {
            let across_first = style < 0.7;
            for leg in 0..2 {
                if (leg == 0) == across_first {
                    while x != z.0 {
                        x += (z.0 - x).signum();
                        dig(x, y);
                    }
                } else {
                    while y != z.1 {
                        y += (z.1 - y).signum();
                        dig(x, y);
                    }
                }
            }
        }
    };
    // Openings: three blocks in the middle of the edge piece, then a corridor
    // in to the hub. (Every brick uses the same three, so they line up.)
    for (k, open) in edges.iter().enumerate() {
        if !*open {
            continue;
        }
        let ((sx, sy), (dx, dy), (nx, ny), h) = pieces[k];
        for along in g / 2 - 1..=g / 2 + 1 {
            for depth in 0..2 {
                set(&mut grid, sx + dx * along + nx * depth, sy + dy * along + ny * depth, Block::Open);
            }
        }
        let port = (sx + dx * (g / 2) + nx * 2, sy + dy * (g / 2) + ny * 2);
        let (wide, style) = (if rnd() < 0.3 { 3 } else { 2 }, rnd());
        corridor(&mut grid, port, hubs[h], wide, style);
    }
    if used[0] && used[1] {
        let style = rnd();
        corridor(&mut grid, hubs[0], hubs[1], 2, style);
    }
    // Dead-end branches.
    for h in 0..2 {
        if used[h] && rnd() < 0.9 {
            let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][(rnd() * 4.0) as usize % 4];
            let len = 2 + (rnd() * 3.0) as i32;
            let (hx, hy) = halves[h];
            let end = ((hubs[h].0 + dx * len).clamp(hx + 1, hx + g - 2), (hubs[h].1 + dy * len).clamp(hy + 1, hy + g - 2));
            corridor(&mut grid, hubs[h], end, 1, rnd());
        }
    }
    // What fills the special room (after the corridors, so they're its doors).
    if let (Some(kind), Some((x0, y0, rw, rh))) = (special, rooms[special_half])
        && used[special_half]
    {
        let floor = y0 + rh - 1;
        match kind {
            // Brick vault: a wooden shelf.
            0 => {
                for x in x0 + 1..x0 + rw - 1 {
                    set(&mut grid, x, y0 + rh / 2, Block::Wood);
                }
            }
            // Stone ring: ore in the middle.
            1 => set(&mut grid, x0 + rw / 2, floor, Block::Ore),
            // Flooded room.
            2 => {
                for y in y0 + rh / 2..=floor {
                    for x in x0..x0 + rw {
                        set(&mut grid, x, y, Block::Water);
                    }
                }
            }
            // Lava pit.
            3 => {
                for x in x0 + 1..x0 + rw - 1 {
                    set(&mut grid, x, floor, Block::Lava);
                }
            }
            // Powder cache.
            _ => {
                for x in x0 + rw / 2 - 1..=x0 + rw / 2 {
                    set(&mut grid, x, floor, Block::Powder);
                }
            }
        }
    }
    // A puddle now and then in a room's floor.
    if s.pools > 0.0 && rnd() < s.pools {
        if let Some((x0, y0, rw, rh)) = rooms.iter().flatten().next().copied() {
            for x in x0..x0 + rw {
                set(&mut grid, x, y0 + rh - 1, Block::Water);
            }
        }
    }

    draw_cave_blocks(world, mats, &grid, (gw, gh, g), at, t, seed, s, inside);
}

/// Distinct natural plans on a finer grid: meanders, caverns, branching
/// trees, pillar loops, chamber chains and narrow fractured shafts. All
/// required doors join the plan; extra branches stop inside the brick.
fn organic_grid(shape: Shape, edges: Edges, seed: u32, pools: f32, dead_ends: f32) -> (Vec<Block>, i32, i32) {
    let (w, h) = match shape { Shape::Horizontal => (32, 16), Shape::Vertical => (16, 32) };
    let mut grid = vec![Block::Ground; (w * h) as usize];
    if !edges.iter().any(|&e| e) { return (grid, w, h); }
    let mut roll = 0;
    let mut rnd = || { roll += 1; unit(hash(seed, roll, 813, 17)) };
    let style = hash(seed, 0, 0, 812) % 6;
    let mut nodes = Vec::new();
    // A spine crosses the whole brick rather than one room per square half.
    // Ring plans leave a substantial central pillar and offer two routes.
    if style == 3 {
        let (cx, cy) = (w as f32 / 2.0 - 0.5 + rnd(), h as f32 / 2.0 - 0.5 + rnd());
        let (rx, ry) = (w as f32 / 2.0 - 5.0 + rnd() * 1.5, h as f32 / 2.0 - 5.0 + rnd() * 1.5);
        for i in 0..8 {
            let angle = i as f32 * std::f32::consts::TAU / 8.0;
            nodes.push((cx + angle.cos() * rx, cy + angle.sin() * ry));
        }
    } else {
        let count = 3 + (rnd() * 4.0) as usize;
        for i in 0..count {
            let along = 3.5 + i as f32 / (count - 1) as f32 * ((w.max(h) - 7) as f32);
            let across = 3.5 + rnd() * 8.0;
            nodes.push(match shape { Shape::Horizontal => (along, across), Shape::Vertical => (across, along) });
        }
    }
    let width = match style { 0 => 1.1, 1 => 1.8, 2 => 1.0, 3 => 1.3, 4 => 1.0, _ => 0.95 };
    for i in 1..nodes.len() {
        let parent = if style == 2 { (rnd() * i as f32) as usize } else { i - 1 };
        dig_route(&mut grid, (w, h), nodes[parent], nodes[i], width, style == 5);
    }
    if style == 3 { dig_route(&mut grid, (w, h), nodes[nodes.len() - 1], nodes[0], width, false); }
    if style == 1 || style == 4 {
        for &(x, y) in &nodes {
            let (rx, ry) = if style == 1 { (5.0 + rnd() * 4.0, 4.0 + rnd() * 2.5) } else { (1.6 + rnd() * 2.0, 1.4 + rnd() * 1.6) };
            dig_ellipse(&mut grid, (w, h), (x, y), (rx, ry));
        }
    }
    let ports: [(i32, i32, i32, i32); 6] = match shape {
        Shape::Horizontal => [(8, 0, 0, 1), (24, 0, 0, 1), (31, 8, -1, 0), (24, 15, 0, -1), (8, 15, 0, -1), (0, 8, 1, 0)],
        Shape::Vertical => [(8, 0, 0, 1), (15, 8, -1, 0), (15, 24, -1, 0), (8, 31, 0, -1), (0, 24, 1, 0), (0, 8, 1, 0)],
    };
    for (k, &(x, y, nx, ny)) in ports.iter().enumerate() {
        if !edges[k] { continue; }
        let entry = ((x + nx * 4) as f32, (y + ny * 4) as f32);
        let near = *nodes.iter().min_by(|a, b| {
            let d = |p: &(f32, f32)| (p.0 - entry.0).powi(2) + (p.1 - entry.1).powi(2);
            d(a).total_cmp(&d(b))
        }).unwrap();
        dig_route(&mut grid, (w, h), entry, near, width.max(1.25), false);
        // The same six-cell opening as the original three-block doors.
        // Write it last, outside the interior-only brush's protected rim.
        for depth in 0..5 {
            for offset in -2..=3 {
                let (px, py) = (x + nx * depth + ny.abs() * offset, y + ny * depth + nx.abs() * offset);
                grid[(py * w + px) as usize] = Block::Open;
            }
        }
    }
    add_blind_branches(&mut grid, (w, h), seed, dead_ends);
    // Small water pockets only where the plan already has a supported floor.
    // Doorway collars are excluded so pools cannot seal a required entrance.
    if rnd() < pools {
        let row = h - 4 - (rnd() * (h - 8) as f32) as i32;
        for x in 3..w - 3 {
            let i = (row * w + x) as usize;
            if grid[i] == Block::Open && grid[i + w as usize] == Block::Ground { grid[i] = Block::Water; }
        }
    }
    (grid, w, h)
}

/// Accept side passages only when their far end stays clear of the existing
/// network. Random offshoots that rejoin another tunnel are loops, not dead
/// ends; the clearance test gives these branches a single mouth and blind tip.
fn add_blind_branches(grid: &mut Vec<Block>, (w, h): (i32, i32), seed: u32, amount: f32) -> usize {
    let target = (amount.clamp(0.0, 1.0) * 5.0).round() as usize;
    let mut made = 0;
    for attempt in 0..160 {
        if made >= target { break; }
        let r = |k| unit(hash(seed, attempt, k, 921));
        let start = (3 + (r(1) * (w - 6) as f32) as i32, 3 + (r(2) * (h - 6) as f32) as i32);
        if grid[(start.1 * w + start.0) as usize] != Block::Open { continue; }
        let angle = r(3) * std::f32::consts::TAU;
        let length = 4.0 + r(4) * 6.0;
        let tip = (start.0 as f32 + angle.cos() * length, start.1 as f32 + angle.sin() * length);
        if tip.0 < 3.0 || tip.1 < 3.0 || tip.0 > (w - 4) as f32 || tip.1 > (h - 4) as f32 { continue; }
        let mut trial = grid.clone();
        dig_route(&mut trial, (w, h), (start.0 as f32, start.1 as f32), tip, 1.0, false);
        dig_ellipse(&mut trial, (w, h), tip, (1.4, 1.4));
        let mut added = 0;
        let mut clear = true;
        for y in 2..h - 2 {
            for x in 2..w - 2 {
                let i = (y * w + x) as usize;
                if trial[i] == grid[i] { continue; }
                added += 1;
                if (x - start.0).pow(2) + (y - start.1).pow(2) <= 12 { continue; }
                for dy in -1..=1 { for dx in -1..=1 {
                    if grid[((y + dy) * w + x + dx) as usize] == Block::Open { clear = false; }
                } }
            }
        }
        if clear && added >= 8 { *grid = trial; made += 1; }
    }
    made
}

/// Elliptical brush, confined to the interior so closed tile edges stay shut.
fn dig_ellipse(grid: &mut [Block], (w, h): (i32, i32), (cx, cy): (f32, f32), (rx, ry): (f32, f32)) {
    for y in (cy - ry).floor().max(2.0) as i32..=((cy + ry).ceil() as i32).min(h - 3) {
        for x in (cx - rx).floor().max(2.0) as i32..=((cx + rx).ceil() as i32).min(w - 3) {
            if ((x as f32 - cx) / rx).powi(2) + ((y as f32 - cy) / ry).powi(2) <= 1.0 {
                grid[(y * w + x) as usize] = Block::Open;
            }
        }
    }
}

/// Connected swept brush; fractured shafts bend at an offset elbow.
fn dig_route(grid: &mut [Block], size: (i32, i32), a: (f32, f32), b: (f32, f32), radius: f32, elbow: bool) {
    if elbow {
        let mid = (a.0, b.1);
        dig_route(grid, size, a, mid, radius, false);
        dig_route(grid, size, mid, b, radius, false);
        return;
    }
    let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 4.0).ceil().max(1.0) as usize;
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        dig_ellipse(grid, size, (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t), (radius, radius));
    }
}

/// Render any coarse cave plan through the same natural-wall sampler.
fn draw_cave_blocks(world: &mut World, mats: &Materials, grid: &[Block], dimensions: (i32, i32, i32), at: (i32, i32), t: usize, seed: u32, s: &Settings, inside: &dyn Fn(i32, i32) -> bool) {
    let (gw, gh, g) = dimensions;
    // Draw rounded natural walls at cell resolution, retaining built details.
    let (pw, ph) = (gw as usize * t / g as usize, gh as usize * t / g as usize);
    let (brick, stone, wood, lava, powder) = (pick(mats, "brick", s.rock), pick(mats, "stone", s.rock), pick(mats, "wood", s.rock), pick(mats, "lava", AIR), pick(mats, "gunpowder", s.rock));
    let ores = [pick(mats, "copper", s.rock), pick(mats, "brass", s.rock), pick(mats, "iron", s.rock)];
    for cy in 0..ph {
        for cx in 0..pw {
            let (x, y) = (at.0 + cx as i32, at.1 + cy as i32);
            if !inside(x, y) { continue; }
            let (sx, sy) = cave_sample(cx, cy, pw, ph, gw as usize, gh as usize, seed);
            let original = grid[(cy * gh as usize / ph) * gw as usize + cx * gw as usize / pw];
            let sampled = natural_sample(grid, gw as usize, gh as usize, sx, sy, Block::Open, Block::Ground);
            let block = keep_feature(original, sampled, Block::Open, Block::Ground);
            let mat = match block {
                Block::Ground => s.rock,
                Block::Open => AIR,
                Block::Brick => brick,
                Block::Stone => stone,
                Block::Water => s.pool,
                Block::Lava => lava,
                Block::Powder => powder,
                Block::Wood => wood,
                Block::Ore => ores[(hash(seed, sx as i32, sy as i32, 5) % 3) as usize],
            };
            let liquid = matches!(block, Block::Water | Block::Lava | Block::Powder);
            let fy = (cy as f32 + 0.5) * gh as f32 / ph as f32;
            let mat = if liquid && fy.fract() < 0.5 { AIR } else { mat };
            world.set(mats, x as usize, y as usize, mat);
        }
    }
}

/// Built features neither bend nor get copied into the surrounding rock.
fn keep_feature<T: Copy + PartialEq>(original: T, sampled: T, air: T, ground: T) -> T {
    if (original != air && original != ground) || (sampled != air && sampled != ground) { original } else { sampled }
}

#[cfg(test)]
mod cave_shape_tests {
    use super::*;

    #[test]
    fn blind_branches_grow_into_rock_and_can_be_disabled() {
        let mut grid = vec![Block::Ground; 48 * 32];
        for y in 14..=16 { for x in 3..45 { grid[y * 48 + x] = Block::Open; } }
        let before = grid.clone();
        assert_eq!(add_blind_branches(&mut grid, (48, 32), 42, 0.0), 0);
        assert!(grid == before);
        assert!(add_blind_branches(&mut grid, (48, 32), 42, 1.0) >= 3);
        assert!(grid.iter().zip(before).filter(|(a, b)| **a != *b).count() >= 24);
    }

    #[test]
    fn large_caverns_span_tiles_and_preserve_owned_cells() {
        let mats = Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"));
        let settings = Settings { tile: 40, caverns: 1.0, seed: 9, ..Settings::new(&mats) };
        let caverns = large_caverns(&settings, (0, 0, 960, 640));
        let c = caverns.iter().find(|c| c.x > c.rx && c.y > c.ry && c.x + c.rx < 960.0 && c.y + c.ry < 640.0).unwrap();
        assert!(c.rx * 2.0 > settings.tile as f32 * 3.0);
        let mut world = World::new(960, 640);
        let (cx, cy) = (c.x as usize, c.y as usize);
        let i = world.idx(cx, cy);
        let wood = mats.id("wood").unwrap();
        world.set(&mats, cx, cy, wood);
        world.flags[i] |= crate::world::OWNED;
        generate(&mut world, &mats, (0, 0, 960, 640), &settings, &[]);
        assert_eq!(world.mat[i], wood);
        // Formations may fill the old core, but the central travel band stays
        // open between the floor pillars and ceiling teeth.
        for y in (c.y - 4.0) as usize..(c.y + 4.0) as usize {
            for x in (c.x - c.rx * 0.45) as usize..(c.x + c.rx * 0.45) as usize {
                if (x, y) != (cx, cy) { assert_eq!(world.mat[world.idx(x, y)], AIR, "large cavern core must be open at {x},{y}"); }
            }
        }
        let off = Settings { caverns: 0.0, ..settings };
        assert!(large_caverns(&off, (0, 0, 960, 640)).is_empty());
    }

    #[test]
    fn large_caverns_have_rooted_formations_and_an_open_middle_route() {
        let mats = Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"));
        let settings = Settings::new(&mats);
        let cavern = LargeCavern { x: 240.0, y: 160.0, rx: 150.0, ry: 85.0, seed: 39 };
        let rocks = cavern_rocks(&cavern, settings.tile);
        for kind in 0..3 { assert!(rocks.iter().any(|r| r.kind == kind), "missing formation type {kind}"); }
        let mut world = World::new(480, 320);
        for y in 0..320 { for x in 0..480 { world.set(&mats, x, y, settings.rock); } }
        carve_large_caverns(&mut world, &mats, (0, 0, 480, 320), (0, 0), &settings, &[cavern], &|_, _| true);
        let cavern = LargeCavern { x: 240.0, y: 160.0, rx: 150.0, ry: 85.0, seed: 39 };
        let mut solid_inside = 0;
        for y in 40..280 { for x in 40..440 {
            if cavern.contains(x as f32, y as f32, settings.tile) {
                if world.mat[world.idx(x, y)] == settings.rock { solid_inside += 1; }
                if (y as i32 - 160).abs() < 8 && (x as i32 - 240).abs() < 60 {
                    assert_eq!(world.mat[world.idx(x, y)], AIR, "formations blocked the travel band");
                }
            }
        } }
        assert!(solid_inside > 500, "cavern must contain real rock ledges, teeth and pillars");
        // Each formation's centreline continues through to the outer wall.
        for r in rocks.iter().filter(|r| r.kind < 2) {
            let sign = if r.kind == 0 { 1.0 } else { -1.0 };
            // The final sub-cell tip may rasterise to no pixel. Check the
            // substantial rooted shaft, rather than rounding that tiny tip.
            for n in 0..(r.height * 0.75) as i32 {
                let x = r.x.round() as usize;
                let y = (r.y + sign * n as f32).round() as usize;
                assert_ne!(world.mat[world.idx(x, y)], AIR, "floating or broken formation");
            }
        }
    }

    #[test]
    fn organic_plans_connect_every_required_door_and_keep_closed_edges_solid() {
        for shape in [Shape::Horizontal, Shape::Vertical] {
            for mask in 1u32..64 {
                for seed in [7, 31, 77] {
                    let edges = std::array::from_fn(|i| mask & (1 << i) != 0);
                    let (grid, w, h) = organic_grid(shape, edges, seed, 0.0, 0.75);
                    let mut seen = vec![false; grid.len()];
                    let first = grid.iter().position(|b| *b == Block::Open).unwrap();
                    let mut stack = vec![first];
                    seen[first] = true;
                    while let Some(i) = stack.pop() {
                        let (x, y) = (i % w as usize, i / w as usize);
                        for (ok, j) in [(x > 0, i.wrapping_sub(1)), (x + 1 < w as usize, i + 1), (y > 0, i.wrapping_sub(w as usize)), (y + 1 < h as usize, i + w as usize)] {
                            if ok && !seen[j] && grid[j] == Block::Open { seen[j] = true; stack.push(j); }
                        }
                    }
                    assert!(grid.iter().enumerate().all(|(i, b)| *b != Block::Open || seen[i]), "disconnected {shape:?}, mask {mask}, seed {seed}");
                    // Outer edge centres are controlled only by their door bit.
                    let ports = match shape {
                        Shape::Horizontal => [(8, 0), (24, 0), (31, 8), (24, 15), (8, 15), (0, 8)],
                        Shape::Vertical => [(8, 0), (15, 8), (15, 24), (8, 31), (0, 24), (0, 8)],
                    };
                    for (k, (x, y)) in ports.into_iter().enumerate() {
                        assert_eq!(grid[(y * w + x) as usize] == Block::Open, edges[k]);
                    }
                }
            }
        }
    }

    #[test]
    fn organic_plans_have_a_wide_range_of_open_space_and_distinct_shapes() {
        let mut counts = Vec::new();
        let mut shapes = std::collections::BTreeSet::new();
        for seed in 0..128 {
            let (grid, _, _) = organic_grid(Shape::Horizontal, [true; 6], seed, 0.0, 0.75);
            counts.push(grid.iter().filter(|b| **b == Block::Open).count());
            shapes.insert(grid.iter().map(|b| *b == Block::Open).collect::<Vec<_>>());
        }
        assert!(shapes.len() > 120, "plans should vary even with the same entrances");
        assert!(counts.iter().max().unwrap() * 10 > counts.iter().min().unwrap() * 15, "tight plans and open caverns should differ substantially: {}..{} open cells", counts.iter().min().unwrap(), counts.iter().max().unwrap());
    }

    #[test]
    fn natural_corners_round_but_a_narrow_passage_stays_open() {
        let mut grid = vec![1; 25];
        for y in 1..4 { for x in 1..4 { grid[y * 5 + x] = 0; } }
        assert_eq!(natural_sample(&grid, 5, 5, 1.1, 1.1, 0, 1), 1);
        assert_eq!(natural_sample(&grid, 5, 5, 1.5, 1.5, 0, 1), 0);
        let mut shaft = vec![1; 25];
        for y in 0..5 { shaft[y * 5 + 2] = 0; }
        for y in 0..50 {
            assert_eq!(natural_sample(&shaft, 5, 5, 2.5, y as f32 / 10.0, 0, 1), 0);
        }
    }

    #[test]
    fn built_features_are_neither_moved_nor_duplicated() {
        assert_eq!(keep_feature(2, 0, 0, 1), 2);
        assert_eq!(keep_feature(0, 2, 0, 1), 0);
        assert_eq!(keep_feature(1, 0, 0, 1), 0);
        for x in 0..128 {
            let (sx, sy) = cave_sample(x, 0, 128, 64, 16, 8, 42);
            assert_eq!(sx, (x as f32 + 0.5) / 8.0);
            assert_eq!(sy, 0.0625);
        }
    }
}

/// Bend natural interiors while keeping the shared doorway collars exact.
/// Brick-local coordinates keep streamed strips identical to whole maps.
fn cave_sample(x: usize, y: usize, w: usize, h: usize, gw: usize, gh: usize, seed: u32) -> (f32, f32) {
    let (u, v) = ((x as f32 + 0.5) * gw as f32 / w as f32, (y as f32 + 0.5) * gh as f32 / h as f32);
    let edge = u.min(v).min(gw as f32 - u).min(gh as f32 - v);
    let fade = ((edge - 1.0) / 1.5).clamp(0.0, 1.0);
    let dx = (noise(seed ^ 0x8123, u * 0.45, v * 0.45) - 0.5) * 1.5 * fade;
    let dy = (noise(seed ^ 0x3917, u * 0.45, v * 0.45) - 0.5) * 1.5 * fade;
    ((u + dx).clamp(0.0, gw as f32 - 0.001), (v + dy).clamp(0.0, gh as f32 - 0.001))
}

/// Round natural corners; masonry, timber and pools keep their authored edges.
fn natural_sample<T: Copy + PartialEq>(grid: &[T], w: usize, h: usize, x: f32, y: f32, air: T, ground: T) -> T {
    let nearest = grid[y as usize * w + x as usize];
    if x < 1.0 || y < 1.0 || x >= w as f32 - 1.0 || y >= h as f32 - 1.0 { return nearest; }
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (ix, iy) = (fx.floor() as usize, fy.floor() as usize);
    let samples = [grid[iy * w + ix], grid[iy * w + ix + 1], grid[(iy + 1) * w + ix], grid[(iy + 1) * w + ix + 1]];
    if samples.iter().any(|&m| m != air && m != ground) { return nearest; }
    let a = samples.map(|m| if m == ground { 1.0 } else { 0.0 });
    let (tx, ty) = (fx.fract(), fy.fract());
    let top = a[0] * (1.0 - tx) + a[1] * tx;
    let bottom = a[2] * (1.0 - tx) + a[3] * tx;
    if top * (1.0 - ty) + bottom * ty >= 0.5 { ground } else { air }
}

/// Smooth noise 0..1 (for patches of different stone).
fn noise(seed: u32, x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let at = |dx: i32, dy: i32| unit(hash(seed, x0 as i32 + dx, y0 as i32 + dy, 11));
    let top = at(0, 0) + (at(1, 0) - at(0, 0)) * sx;
    let bot = at(0, 1) + (at(1, 1) - at(0, 1)) * sx;
    top + (bot - top) * sy
}

/// Patches of other stone through the plain ground, and specks of ore in
/// the walls next to passages (the coloured dots that make a map readable).
fn texture_ground(world: &mut World, mats: &Materials, area: (usize, usize, usize, usize), origin: (i64, i64), s: &Settings, free: &dyn Fn(i32, i32) -> bool) {
    let (ax, ay, aw, ah) = area;
    // Map position of a world cell (patches and ore follow the map).
    let map = |x: usize, y: usize| ((origin.0 + x as i64) as i32, (origin.1 + y as i64) as i32);
    let (stone, sand) = (pick(mats, "shale", s.rock), pick(mats, "sandstone", s.rock));
    let ores = [pick(mats, "copper", s.rock), pick(mats, "brass", s.rock), pick(mats, "iron", s.rock)];
    let scale = (s.tile as f32 * 1.5).max(10.0);
    for y in ay..ay + ah {
        for x in ax..ax + aw {
            let i = world.idx(x, y);
            if world.mat[i] != s.rock || !free(x as i32, y as i32) {
                continue;
            }
            let (mx, my) = map(x, y);
            let (fx, fy) = (mx as f32, my as f32);
            let n = noise(s.seed ^ 0x5151, fx / scale, fy / scale) * 0.75 + noise(s.seed ^ 0x7373, fx / (scale * 0.3), fy / (scale * 0.3)) * 0.25;
            let m = if n > 0.66 { stone } else if n < 0.17 { sand } else { s.rock };
            if m != s.rock {
                world.set(mats, x, y, m);
            }
        }
    }
    // Ore: small clumps (a 2x2 in a 3x3 grid square, where the map says
    // so). Decided cell by cell from the map position alone, so it comes out
    // the same however the map is cut into regions; clumps show where
    // passages cut through them.
    for y in ay..ay + ah {
        for x in ax..ax + aw {
            let (mx, my) = map(x, y);
            let (gx, gy) = (mx.div_euclid(3) * 3, my.div_euclid(3) * 3);
            if mx - gx > 1 || my - gy > 1 || unit(hash(s.seed, gx, gy, 77)) > 0.012 || !free(x as i32, y as i32) {
                continue;
            }
            let i = world.idx(x, y);
            // (Only in the plain ground, not in a tile's brick or timber.)
            if world.mat[i] == s.rock || world.mat[i] == stone || world.mat[i] == sand {
                world.set(mats, x, y, ores[(hash(s.seed, gx, gy, 78) % 3) as usize]);
            }
        }
    }
}
