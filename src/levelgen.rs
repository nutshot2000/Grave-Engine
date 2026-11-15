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
    /// The wall drawn behind the caves (`AIR` = none: the sky shows).
    pub back: MatId,
    /// How often a tile gets a special room (vault, ore ring, flooded room,
    /// lava pit, powder cache), 0..1.
    pub rooms: f32,
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
    // Varied ground: patches of other stone, and ore specks in the walls.
    texture_ground(world, mats, (ax, ay, aw, ah), s, &free);
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
        if used[h] && rnd() < 0.8 {
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
        if used[h] && rnd() < 0.55 {
            let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][(rnd() * 4.0) as usize % 4];
            let len = 2 + (rnd() * 3.0) as i32;
            let (hx, hy) = halves[h];
            let end = ((hubs[h].0 + dx * len).clamp(hx + 1, hx + g - 2), (hubs[h].1 + dy * len).clamp(hy + 1, hy + g - 2));
            corridor(&mut grid, hubs[h], end, 2, 0.5);
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

    // Draw it: each block is a square of cells.
    let px = (t as i32 / g).max(1);
    let (brick, stone, wood, lava, powder) = (pick(mats, "brick", s.rock), pick(mats, "stone", s.rock), pick(mats, "wood", s.rock), pick(mats, "lava", AIR), pick(mats, "gunpowder", s.rock));
    let ores = [pick(mats, "copper", s.rock), pick(mats, "brass", s.rock), pick(mats, "iron", s.rock)];
    for by in 0..gh {
        for bx in 0..gw {
            let block = grid[(by * gw + bx) as usize];
            let mat = match block {
                Block::Ground => continue,
                Block::Open => AIR,
                Block::Brick => brick,
                Block::Stone => stone,
                Block::Water => s.pool,
                Block::Lava => lava,
                Block::Powder => powder,
                Block::Wood => wood,
                Block::Ore => ores[(hash(seed, bx, by, 5) % 3) as usize],
            };
            // Liquids and powder only fill the lower half of their block (room to splash).
            let liquid = matches!(block, Block::Water | Block::Lava | Block::Powder);
            for cy in 0..px {
                for cx in 0..px {
                    let (x, y) = (at.0 + bx * px + cx, at.1 + by * px + cy);
                    if inside(x, y) {
                        let m = if liquid && cy < px / 2 { AIR } else { mat };
                        world.set(mats, x as usize, y as usize, m);
                    }
                }
            }
        }
    }
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
fn texture_ground(world: &mut World, mats: &Materials, area: (usize, usize, usize, usize), s: &Settings, free: &dyn Fn(i32, i32) -> bool) {
    let (ax, ay, aw, ah) = area;
    let (stone, sand) = (pick(mats, "stone", s.rock), pick(mats, "sandstone", s.rock));
    let ores = [pick(mats, "copper", s.rock), pick(mats, "brass", s.rock), pick(mats, "iron", s.rock)];
    let scale = (s.tile as f32 * 1.5).max(10.0);
    for y in ay..ay + ah {
        for x in ax..ax + aw {
            let i = world.idx(x, y);
            if world.mat[i] != s.rock || !free(x as i32, y as i32) {
                continue;
            }
            let n = noise(s.seed ^ 0x5151, x as f32 / scale, y as f32 / scale) * 0.75 + noise(s.seed ^ 0x7373, x as f32 / (scale * 0.3), y as f32 / (scale * 0.3)) * 0.25;
            let m = if n > 0.66 { stone } else if n < 0.17 { sand } else { s.rock };
            if m != s.rock {
                world.set(mats, x, y, m);
            }
        }
    }
    // Ore: small clumps in walls that face a passage.
    let mut seeds = Vec::new();
    for y in (ay + 1..ay + ah - 1).step_by(3) {
        for x in (ax + 1..ax + aw - 1).step_by(3) {
            let i = world.idx(x, y);
            if world.mat[i] == AIR || !free(x as i32, y as i32) || unit(hash(s.seed, x as i32, y as i32, 77)) > 0.035 {
                continue;
            }
            let near_open = [i - 1, i + 1, i - world.w, i + world.w].iter().any(|&j| world.mat[j] == AIR);
            if near_open && mats.get(world.mat[i]).state == crate::material::State::Solid {
                seeds.push((x, y, ores[(hash(s.seed, x as i32, y as i32, 78) % 3) as usize]));
            }
        }
    }
    for (x, y, ore) in seeds {
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1), (-1, 0)] {
            let (cx, cy) = (x as i32 + dx, y as i32 + dy);
            if cx >= ax as i32 && cy >= ay as i32 && cx < (ax + aw) as i32 && cy < (ay + ah) as i32 && free(cx, cy) {
                let i = world.idx(cx as usize, cy as usize);
                if world.mat[i] != AIR && mats.get(world.mat[i]).state == crate::material::State::Solid {
                    world.set(mats, cx as usize, cy as usize, ore);
                }
            }
        }
    }
}
