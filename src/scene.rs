//! Scenes are plain text. You can draw with shape commands, paste a grid of
//! characters, or both. Later lines paint over earlier ones.
//!
//! ```text
//! # A little test scene
//! size 512 288                 # width height (cells)
//! seed 7                       # random seed (same seed = same result)
//! fill air
//! rect stone 0 270 512 18      # material x y width height
//! circle water 100 200 20      # material centre-x centre-y radius
//! line wood 10 10 50 50 3      # material x1 y1 x2 y2 thickness
//! heat circle 120 250 6 900    # set temperature (°C) in a circle
//! heat rect 0 0 10 10 -40      # ...or in a rectangle
//! legend . air                 # grid characters
//! legend S stone
//! grid 0 260                   # optional top-left corner, then one line per row
//! ......SSSS......
//! end
//!
//! # Objects: solid things that fall, tumble and break apart
//! body rect wood 100 50 20 10        # material x y width height
//! body circle stone 200 40 6         # material centre-x centre-y radius
//! object assets/crate.png 300 20     # an image or Blender model (.png .glb .gltf), top-left at x y
//! object assets/cart.glb 50 30 scale 12 static   # `static`: paint it in place instead
//! body 120.5 40.0 15                 # a saved object: centre x y, angle in degrees,
//! WWWW                               # then its cells using the legend
//! W..W
//! end
//! ```

use crate::import;
use crate::material::{AIR, MatId, Materials};
use crate::physics::BodySpec;
use crate::sim::Snapshot;
use crate::world::World;
use std::collections::HashMap;
use std::path::Path;

/// Everything a scene file describes.
pub struct Loaded {
    pub world: World,
    /// Objects (rigid bodies) to add.
    pub bodies: Vec<BodySpec>,
    /// Problems found, with line numbers. The rest of the scene still loads.
    pub problems: Vec<String>,
    /// Cells set by `heat` commands, re-applied to objects once they're placed.
    pub heated: Vec<(usize, f32)>,
}

impl Loaded {
    pub fn into_sim(self, mats: &Materials) -> (crate::sim::Sim, Vec<String>) {
        let mut sim = crate::sim::Sim::new(self.world, self.bodies, mats);
        for (i, t) in self.heated {
            if sim.world.is_owned(i) {
                sim.world.temp[i] = t;
            }
        }
        (sim, self.problems)
    }
}

/// Read rows of characters up to `end`, as material ids.
fn read_rows<'a>(
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
    legend: &mut HashMap<char, MatId>,
    problems: &mut Vec<String>,
) -> (Vec<Vec<Option<MatId>>>, bool) {
    let mut rows = Vec::new();
    for (_, row) in lines.by_ref() {
        let row = row.trim_end();
        if row.trim() == "end" {
            return (rows, true);
        }
        let mut out = Vec::new();
        for c in row.chars() {
            out.push(match legend.get(&c) {
                Some(&AIR) => None,
                Some(&id) => Some(id),
                None if c == ' ' => None,
                None => {
                    problems.push(format!("scene: no legend for `{c}`"));
                    legend.insert(c, AIR);
                    None
                }
            });
        }
        rows.push(out);
    }
    (rows, false)
}

fn rows_to_grid(rows: &[Vec<Option<MatId>>]) -> (usize, usize, Vec<Option<MatId>>) {
    let w = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let h = rows.len();
    let mut cells = vec![None; w * h];
    for (y, r) in rows.iter().enumerate() {
        for (x, c) in r.iter().enumerate() {
            cells[y * w + x] = *c;
        }
    }
    (w, h, cells)
}

pub const DEFAULT_W: usize = 512;
pub const DEFAULT_H: usize = 288;

/// Build a world from scene text. Problems are reported per line; the rest of
/// the scene still loads.
/// `base` is the folder that relative `object` file paths start from
/// (usually the scene file's folder).
pub fn load(text: &str, mats: &Materials, base: Option<&Path>) -> Loaded {
    let mut problems = Vec::new();
    let mut bodies: Vec<BodySpec> = Vec::new();
    let mut heated: Vec<(usize, f32)> = Vec::new();
    let mut world: Option<World> = None;
    let mut legend: HashMap<char, MatId> = HashMap::new();
    legend.insert('.', AIR);
    let mut lines = text.lines().enumerate().peekable();

    fn ensure<'a>(world: &'a mut Option<World>, mats: &Materials) -> &'a mut World {
        world.get_or_insert_with(|| {
            let mut w = World::new(DEFAULT_W, DEFAULT_H);
            w.fill(mats, AIR);
            w
        })
    }

    while let Some((n, raw)) = lines.next() {
        let line_no = n + 1;
        let line = crate::material::strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let cmd = words[0].to_lowercase();
        let mut err = |msg: String| problems.push(format!("scene line {line_no}: {msg}"));

        let mat = |name: &str| -> Result<MatId, String> {
            mats.id(name)
                .ok_or_else(|| format!("there is no material called `{name}`"))
        };
        let nums = |from: usize, count: usize| -> Result<Vec<f32>, String> {
            if words.len() < from + count {
                return Err(format!("`{cmd}` needs {count} numbers"));
            }
            words[from..from + count]
                .iter()
                .map(|w| w.parse::<f32>().map_err(|_| format!("`{w}` is not a number")))
                .collect()
        };

        match cmd.as_str() {
            "size" => match nums(1, 2) {
                Ok(v) => {
                    let (w, h) = (v[0].clamp(8.0, 4096.0) as usize, v[1].clamp(8.0, 4096.0) as usize);
                    let mut nw = World::new(w, h);
                    nw.fill(mats, AIR);
                    if let Some(old) = &world {
                        nw.seed(old.frame);
                    }
                    world = Some(nw);
                }
                Err(e) => err(e),
            },
            "seed" => match nums(1, 1) {
                Ok(v) => ensure(&mut world, mats).seed(v[0] as u32),
                Err(e) => err(e),
            },
            "ambient" => match nums(1, 1) {
                Ok(v) => ensure(&mut world, mats).ambient = v[0],
                Err(e) => err(e),
            },
            "fill" => match words.get(1).ok_or("`fill` needs a material".to_string()).and_then(|m| mat(m)) {
                Ok(id) => ensure(&mut world, mats).fill(mats, id),
                Err(e) => err(e),
            },
            "rect" | "circle" | "line" => {
                let count = match cmd.as_str() {
                    "rect" => 4,
                    "circle" => 3,
                    _ => 4,
                };
                let r = words
                    .get(1)
                    .ok_or(format!("`{cmd}` needs a material"))
                    .and_then(|m| mat(m))
                    .and_then(|id| nums(2, count).map(|v| (id, v)));
                match r {
                    Ok((id, v)) => {
                        let w = ensure(&mut world, mats);
                        let v: Vec<i32> = v.iter().map(|x| *x as i32).collect();
                        match cmd.as_str() {
                            "rect" => rect(w, mats, v[0], v[1], v[2], v[3], id),
                            "circle" => w.paint_circle(mats, v[0], v[1], v[2], id),
                            _ => {
                                let thick = words.get(6).and_then(|t| t.parse::<i32>().ok()).unwrap_or(1);
                                line_shape(w, mats, v[0], v[1], v[2], v[3], thick, id);
                            }
                        }
                    }
                    Err(e) => err(e),
                }
            }
            "heat" => {
                let shape = words.get(1).map(|s| s.to_lowercase()).unwrap_or_default();
                let r = match shape.as_str() {
                    "rect" => nums(2, 5),
                    "circle" => nums(2, 4),
                    _ => Err("use `heat rect x y w h temp` or `heat circle x y r temp`".into()),
                };
                match r {
                    Ok(v) => {
                        let w = ensure(&mut world, mats);
                        let t = *v.last().unwrap();
                        let iv: Vec<i32> = v.iter().map(|x| *x as i32).collect();
                        let (x0, y0, x1, y1, circle) = if shape == "rect" {
                            (iv[0], iv[1], iv[0] + iv[2] - 1, iv[1] + iv[3] - 1, None)
                        } else {
                            (iv[0] - iv[2], iv[1] - iv[2], iv[0] + iv[2], iv[1] + iv[2], Some((iv[0], iv[1], iv[2])))
                        };
                        for y in y0..=y1 {
                            for x in x0..=x1 {
                                if !w.in_bounds(x, y) {
                                    continue;
                                }
                                if let Some((cx, cy, r)) = circle {
                                    if (x - cx).pow(2) + (y - cy).pow(2) > r * r + r / 2 {
                                        continue;
                                    }
                                }
                                let i = w.idx(x as usize, y as usize);
                                w.temp[i] = t;
                                heated.push((i, t));
                            }
                        }
                    }
                    Err(e) => err(e),
                }
            }
            "legend" => {
                let ch = words.get(1).and_then(|c| {
                    let mut it = c.chars();
                    let first = it.next();
                    if it.next().is_none() { first } else { None }
                });
                match (ch, words.get(2)) {
                    (Some(c), Some(name)) => match mat(name) {
                        Ok(id) => {
                            legend.insert(c, id);
                        }
                        Err(e) => err(e),
                    },
                    _ => err("use `legend S stone` (one character, then a material)".into()),
                }
            }
            "grid" => {
                let x0 = words.get(1).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
                let y0 = words.get(2).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
                let w = ensure(&mut world, mats);
                let mut y = y0;
                let mut ended = false;
                for (_, row) in lines.by_ref() {
                    let row = row.trim_end();
                    if row.trim() == "end" {
                        ended = true;
                        break;
                    }
                    for (dx, c) in row.chars().enumerate() {
                        let x = x0 + dx as i32;
                        if !w.in_bounds(x, y) {
                            continue;
                        }
                        match legend.get(&c) {
                            Some(&id) => w.set(mats, x as usize, y as usize, id),
                            None if c == ' ' => {}
                            None => {
                                problems.push(format!("scene grid row {}: no legend for `{c}`", y - y0));
                                legend.insert(c, AIR);
                            }
                        }
                    }
                    y += 1;
                }
                if !ended {
                    problems.push(format!("scene line {line_no}: `grid` is missing its `end` line"));
                }
            }
            "body" => {
                let shape = words.get(1).map(|s| s.to_lowercase()).unwrap_or_default();
                if shape == "rect" || shape == "circle" {
                    let count = if shape == "rect" { 4 } else { 3 };
                    let r = words
                        .get(2)
                        .ok_or(format!("`body {shape}` needs a material"))
                        .and_then(|m| mat(m))
                        .and_then(|id| nums(3, count).map(|v| (id, v)));
                    match r {
                        Ok((id, v)) => {
                            let (w, h, cells, cx, cy) = if shape == "rect" {
                                let (w, h) = (v[2].max(1.0) as usize, v[3].max(1.0) as usize);
                                (w, h, vec![Some(id); w * h], v[0] + w as f32 / 2.0, v[1] + h as f32 / 2.0)
                            } else {
                                let r = v[2].max(0.0) as i32;
                                let d = (2 * r + 1) as usize;
                                let mut cells = vec![None; d * d];
                                for y in -r..=r {
                                    for x in -r..=r {
                                        if x * x + y * y <= r * r + r / 2 {
                                            cells[((y + r) as usize) * d + (x + r) as usize] = Some(id);
                                        }
                                    }
                                }
                                (d, d, cells, v[0] + 0.5, v[1] + 0.5)
                            };
                            bodies.push(BodySpec::from_mats(w, h, &cells, mats, cx, cy));
                        }
                        Err(e) => err(e),
                    }
                } else {
                    match nums(1, 2) {
                        Ok(v) => {
                            let angle = words.get(3).and_then(|a| a.parse::<f32>().ok()).unwrap_or(0.0);
                            let (rows, ended) = read_rows(&mut lines, &mut legend, &mut problems);
                            if !ended {
                                problems.push(format!("scene line {line_no}: `body` is missing its `end` line"));
                            }
                            let (w, h, cells) = rows_to_grid(&rows);
                            let mut spec = BodySpec::from_mats(w, h, &cells, mats, v[0], v[1]);
                            spec.angle = angle.to_radians();
                            if spec.cell_count() > 0 {
                                bodies.push(spec);
                            }
                        }
                        Err(_) => err("use `body rect wood x y w h`, `body circle wood x y r`, or `body x y angle` + rows + `end`".into()),
                    }
                }
            }
            "object" => {
                let Some(file) = words.get(1) else {
                    err("use `object file.png x y` (also .glb / .gltf)".into());
                    continue;
                };
                let pos = nums(2, 2);
                let rest: Vec<String> = words.iter().skip(4).map(|w| w.to_lowercase()).collect();
                let is_static = rest.iter().any(|w| w == "static");
                let opt = |key: &str| -> Option<f32> {
                    rest.iter().position(|w| w == key).and_then(|i| rest.get(i + 1)).and_then(|v| v.parse().ok())
                };
                let path = match base.map(|b| b.join(file)) {
                    Some(p) if p.exists() => p,
                    _ => Path::new(file).to_path_buf(),
                };
                match pos.and_then(|v| import::load(&path, mats, opt("scale")).map(|r| (v, r))) {
                    Ok((v, (shape, warnings))) => {
                        for w in warnings {
                            problems.push(format!("scene line {line_no}: {w}"));
                        }
                        let (x0, y0) = (v[0], v[1]);
                        if is_static {
                            let w = ensure(&mut world, mats);
                            for y in 0..shape.h {
                                for x in 0..shape.w {
                                    if let Some(id) = shape.cells[y * shape.w + x] {
                                        let (wx, wy) = (x0 as i32 + x as i32, y0 as i32 + y as i32);
                                        if w.in_bounds(wx, wy) {
                                            w.set(mats, wx as usize, wy as usize, id);
                                        }
                                    }
                                }
                            }
                        } else {
                            let mut spec = BodySpec::from_mats(
                                shape.w,
                                shape.h,
                                &shape.cells,
                                mats,
                                x0 + shape.w as f32 / 2.0,
                                y0 + shape.h as f32 / 2.0,
                            );
                            spec.angle = opt("angle").unwrap_or(0.0).to_radians();
                            bodies.push(spec);
                        }
                    }
                    Err(e) => err(e),
                }
            }
            _ => err(format!("unknown command `{cmd}`")),
        }
    }
    let world = ensure(&mut world, mats).clone();
    Loaded { world, bodies, problems, heated }
}

fn rect(w: &mut World, mats: &Materials, x: i32, y: i32, rw: i32, rh: i32, id: MatId) {
    for yy in y..y + rh {
        for xx in x..x + rw {
            if w.in_bounds(xx, yy) {
                w.set(mats, xx as usize, yy as usize, id);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn line_shape(w: &mut World, mats: &Materials, x1: i32, y1: i32, x2: i32, y2: i32, thick: i32, id: MatId) {
    let steps = (x2 - x1).abs().max((y2 - y1).abs()).max(1);
    let r = (thick.max(1) - 1) / 2;
    for s in 0..=steps {
        let x = x1 + (x2 - x1) * s / steps;
        let y = y1 + (y2 - y1) * s / steps;
        if r == 0 {
            if w.in_bounds(x, y) {
                w.set(mats, x as usize, y as usize, id);
            }
        } else {
            w.paint_circle(mats, x, y, r, id);
        }
    }
}

/// Write a world out as scene text (materials as a character grid).
/// Temperatures aren't saved: cells start at their material's normal temperature.
pub fn save(snap: &Snapshot, mats: &Materials) -> String {
    let world = &snap.world;
    let mut chars: HashMap<MatId, char> = HashMap::new();
    let mut used: Vec<char> = vec!['.'];
    chars.insert(AIR, '.');
    let pool: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789%&*+=?@$~^!:;<>/|()[]{}-_,"
        .chars()
        .collect();
    let mut present: Vec<MatId> = world.mat.clone();
    for b in &snap.bodies {
        present.extend(b.cells.iter().flatten().map(|c| c.mat));
    }
    present.sort_unstable();
    present.dedup();
    let mut out = String::new();
    out.push_str("# Grave Engine scene. Plain text: edit freely.\n");
    out.push_str(&format!("size {} {}\n", world.w, world.h));
    if world.ambient != crate::world::ROOM_TEMP {
        out.push_str(&format!("ambient {}\n", world.ambient));
    }
    for id in present {
        if id == AIR {
            continue;
        }
        let name = &mats.get(id).name;
        let first = name.chars().next().unwrap_or('X');
        let pick = [first.to_ascii_uppercase(), first.to_ascii_lowercase()]
            .into_iter()
            .chain(pool.iter().copied())
            .find(|c| !used.contains(c))
            .unwrap_or('?');
        used.push(pick);
        chars.insert(id, pick);
    }
    out.push_str("legend . air\n");
    let mut legend: Vec<(char, MatId)> = chars.iter().filter(|(id, _)| **id != AIR).map(|(id, c)| (*c, *id)).collect();
    legend.sort();
    for (c, id) in legend {
        out.push_str(&format!("legend {c} {}\n", mats.get(id).name));
    }
    out.push_str("grid\n");
    for y in 0..world.h {
        for x in 0..world.w {
            let i = world.idx(x, y);
            out.push(if world.is_owned(i) { '.' } else { chars[&world.mat[i]] });
        }
        out.push('\n');
    }
    out.push_str("end\n");
    if !snap.bodies.is_empty() {
        out.push_str("\n# Objects: centre x y, angle in degrees, then their cells\n");
    }
    for b in &snap.bodies {
        out.push_str(&format!("body {:.2} {:.2} {:.1}\n", b.x, b.y, b.angle.to_degrees()));
        for y in 0..b.h {
            for x in 0..b.w {
                out.push(b.cells[y * b.w + x].map(|c| chars[&c.mat]).unwrap_or('.'));
            }
            out.push('\n');
        }
        out.push_str("end\n");
    }
    out
}
