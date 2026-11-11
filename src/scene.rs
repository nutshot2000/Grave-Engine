//! Scenes are plain text. You can draw with shape commands, paste a grid of
//! characters, or both. Later lines paint over earlier ones.
//!
//! ```text
//! # A little test scene
//! size 512 288                 # width height (cells)
//! seed 7                       # random seed (same seed = same result)
//! wind 3                       # steady wind in m/s (+ blows right, - left)
//! turbulence 1                 # how swirly the air is (0 still, 1 normal, 3 stormy)
//! fill air
//! rect stone 0 270 512 18      # material x y width height
//! circle water 100 200 20      # material centre-x centre-y radius
//! line wood 10 10 50 50 3      # material x1 y1 x2 y2 thickness
//!
//! # Level shapes: each fills solid ground below its top surface (down to the
//! # bottom of the world, or `to Y`), optionally with a `cover MAT DEPTH` layer
//! slope stone 170 210 255 178              # a straight ramp/landing: from x1,y1 to x2,y2
//! hill sandstone 440 690 38 210 cover sand 5   # a smooth hill: x1 x2 height base-y
//! curve stone 100 210 140 210 160 170      # a curved surface: x1 y1, bend point, x2 y2
//! poly brick 10 10 40 10 25 30             # any filled shape: three or more x y points
//! anchor 1040 160 90 10                    # pin scenery in this box: it never collapses
//!
//! # Lighting (see light.rs): overall light and tint, a sun, lamps
//! lighting 0.25 #7080c0                  # a blue night (1 = full daylight, the default)
//! sun 35 0.9 #ffe0b0                     # sunlight from 35° left of straight down, with shadows
//! light 120 80 60 strength 1.2 color #ffb060 flicker 0.3   # a torch lighting 60 cells around
//! light 300 150 90 cone 0 25 on bike     # a headlight on `bike` pointing right, 25° either side
//!
//! # Backdrop (see backdrop.rs): a sky and parallax layers, far to near
//! sky #1b2a4a #d07050 stars
//! layer mountains #40486a depth 0.15 y 150 height 90 snow
//! layer city #1f2430 depth 0.5 y 200 height 70 windows
//!
//! # Triggers: a box that does things when something enters (see trigger.rs)
//! trigger 1535 150 10 60 by bike : heat circle 1508 205 8 1200 ; message DEMOLITION CHARGE!
//! trigger 1340 100 10 120 by bike : checkpoint ; score 500
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
//! object rock.glb 50 30 material rock  # make all of it one material
//! place crate 200 60 angle 15 material iron flip   # a copy of templates/crate.txt
//! name crate1                       # give the last object a name (for joints)
//!
//! depth 0.02                       # previous body thickness in metres (default 1)
//! # Joints between named objects (or `ground`); `at x y` is where, default the second's centre
//! hinge door ground at 40 100 motor 0.5 strength 2 # a pin; optional motor (turns/s, kN·m)
//! wheel car back at 100 210 spring 60 damping 4 travel 3 motor 2 strength 4 drive
//! weld car cargo at 120 200                # glued together
//! rope lamp ground at 50 40 to 50 10 length 30
//! weld car cargo break 25           # optional attachment failure load in kN
//! follow car zoom 3                  # the camera follows `car`, 3 times closer than the whole world
//! upright bike strength 60           # a balance assist keeps `bike` (and what's welded/wheeled to it) upright
//! player 40 260 look hero zoom 3     # the player, feet at x y; `look` = assets/characters/hero
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
    /// For each object in `bodies`: the template it's a copy of, if any.
    pub instances: Vec<Option<crate::template::Instance>>,
    /// Joints to make once the objects exist.
    pub joints: Vec<PendingJoint>,
    /// The object the camera follows, and how close (0 = editor decides).
    pub follow: Option<(usize, f32)>,
    /// Objects kept upright by a balance assist: (object number, strength).
    pub upright: Vec<(usize, crate::physics::Balance)>,
    /// Triggers (`by` an object = its number N+1, like joints).
    pub triggers: Vec<crate::trigger::Trigger>,
    /// Lighting (lamps `on` an object = its number N+1).
    pub lighting: crate::light::Lighting,
    pub backdrop: crate::backdrop::Backdrop,
    /// Characters (the player), and how close the camera follows them.
    pub actors: Vec<crate::physics::ActorSpec>,
    pub player_zoom: f32,
}

/// One end of a joint in a scene: the ground, or object number N in `bodies`.
#[derive(Clone, Copy, Debug)]
pub enum End {
    Ground,
    Object(usize),
}

/// A joint line from a scene, made once the objects exist.
#[derive(Clone, Debug)]
pub struct PendingJoint {
    pub a: End,
    pub b: End,
    pub kind: crate::physics::JointKind,
    /// Where (world cells). None: the second object's centre.
    pub at: Option<(f32, f32)>,
    /// Rope: the other end's position.
    pub to: Option<(f32, f32)>,
    pub motor: Option<crate::physics::Motor>,
    pub break_force: Option<f32>,
}

impl Loaded {
    pub fn into_sim(self, mats: &Materials) -> (crate::sim::Sim, Vec<String>) {
        use crate::physics::{GROUND, JointKind};
        let mut problems = self.problems;
        let mut sim = crate::sim::Sim::new_with(self.world, Vec::new(), Vec::new(), mats);
        let mut ids = Vec::new();
        let links = self.instances.into_iter().chain(std::iter::repeat(None));
        for (b, inst) in self.bodies.into_iter().zip(links) {
            let id = sim.physics.add(&mut sim.world, mats, b);
            if let (Some(id), Some(inst)) = (id, inst) {
                sim.instances.insert(id, inst);
            }
            ids.push(id);
        }
        for j in self.joints {
            let id_of = |e: End| match e {
                End::Ground => Some(GROUND),
                End::Object(n) => ids.get(n).copied().flatten(),
            };
            let (Some(a), Some(b)) = (id_of(j.a), id_of(j.b)) else {
                problems.push("scene: a joint's object is missing".into());
                continue;
            };
            let centre = |id: u64| sim.physics.spec_of(&sim.world, id).map(|s| (s.x, s.y));
            let at = j.at.or_else(|| centre(b)).or_else(|| centre(a)).unwrap_or((0.0, 0.0));
            let to = match j.kind {
                JointKind::Rope { .. } => Some(j.to.or_else(|| centre(b)).unwrap_or(at)),
                _ => None,
            };
            let at = match j.kind {
                JointKind::Rope { .. } => j.at.or_else(|| centre(a)).unwrap_or(at),
                _ => at,
            };
            // A rope with no length given is as long as the gap.
            let kind = match j.kind {
                JointKind::Rope { length } if length <= 0.0 => {
                    let t = to.unwrap_or(at);
                    JointKind::Rope { length: ((t.0 - at.0).powi(2) + (t.1 - at.1).powi(2)).sqrt() }
                }
                k => k,
            };
            if !sim.physics.join(mats, a, b, kind, at, to, j.motor) {
                problems.push("scene: couldn't make a joint".into());
            } else if let Some(force) = j.break_force
                && !sim.physics.set_joint_break_force(a, b, force) { problems.push("scene: break force must be finite and positive".into()); }
        }
        let mut sim = sim;
        for a in self.actors {
            sim.physics.add_actor(&mut sim.world, mats, a);
        }
        if self.player_zoom > 0.0 {
            sim.zoom = self.player_zoom;
        }
        sim.lighting = self.lighting;
        sim.backdrop = self.backdrop;
        sim.lighting.lights.retain_mut(|l| match l.on {
            Some(n) => match ids.get(n as usize - 1).copied().flatten() {
                Some(id) => {
                    l.on = Some(id);
                    true
                }
                None => false,
            },
            None => true,
        });
        for mut t in self.triggers {
            if let crate::trigger::By::Object(n) = t.by {
                match ids.get(n as usize - 1).copied().flatten() {
                    Some(id) => t.by = crate::trigger::By::Object(id),
                    None => {
                        problems.push("scene: a trigger's object is missing".into());
                        continue;
                    }
                }
            }
            sim.triggers.push(t);
        }
        for (n, bal) in self.upright {
            if let Some(id) = ids.get(n).copied().flatten() {
                sim.physics.set_balance(id, bal);
            }
        }
        if let Some((n, zoom)) = self.follow {
            sim.follow = ids.get(n).copied().flatten();
            sim.zoom = zoom;
        }
        let problems_out = problems;
        for (i, t) in self.heated {
            if sim.world.is_owned(i) {
                sim.world.temp[i] = t;
            }
        }
        sim.world.wake_all();
        (sim, problems_out)
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
    load_part(text, mats, base, true)
}

/// `load`, optionally refusing `place` (template files can't place templates).
pub fn load_part(text: &str, mats: &Materials, base: Option<&Path>, allow_place: bool) -> Loaded {
    let mut problems = Vec::new();
    let mut bodies: Vec<BodySpec> = Vec::new();
    let mut placed: Vec<(usize, crate::template::Instance)> = Vec::new();
    let mut names: HashMap<String, usize> = HashMap::new();
    let mut joints: Vec<PendingJoint> = Vec::new();
    let mut follow: Option<(usize, f32)> = None;
    let mut upright: Vec<(usize, crate::physics::Balance)> = Vec::new();
    let mut triggers: Vec<crate::trigger::Trigger> = Vec::new();
    let mut lighting = crate::light::Lighting::default();
    let mut backdrop = crate::backdrop::Backdrop::default();
    let mut actors: Vec<crate::physics::ActorSpec> = Vec::new();
    let mut player_zoom = 0.0f32;
    let mut template_cache: HashMap<String, crate::template::Template> = HashMap::new();
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
                    let (w, h) = (v[0].clamp(8.0, 8192.0) as usize, v[1].clamp(8.0, 8192.0) as usize);
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
            "wind" => match nums(1, 1) {
                Ok(v) => ensure(&mut world, mats).wind = v[0].clamp(-50.0, 50.0),
                Err(e) => err(e),
            },
            "turbulence" => match nums(1, 1) {
                Ok(v) => ensure(&mut world, mats).turbulence = v[0].clamp(0.0, 10.0),
                Err(e) => err(e),
            },
            "sky" | "layer" => {
                if let Err(e) = backdrop.parse_line(&words) {
                    err(e);
                }
                // Pictures are found next to the scene (or from the project folder).
                backdrop.dir = base.map(|b| b.to_path_buf());
            }
            "lighting" => match nums(1, 1) {
                Ok(v) => {
                    lighting.ambient = v[0].clamp(0.0, 1.5);
                    if let Some(c) = words.get(2) {
                        match crate::material::parse_color(c) {
                            Ok(c) => lighting.ambient_color = c,
                            Err(e) => err(e),
                        }
                    }
                }
                Err(_) => err("use `lighting LEVEL [#colour]` (0 = dark, 1 = full daylight)".into()),
            },
            "sun" => {
                if words.get(1).is_some_and(|w| w.eq_ignore_ascii_case("off")) {
                    lighting.sun = None;
                } else {
                    match nums(1, 2) {
                        Ok(v) => {
                            let color = match words.get(3).map(|c| crate::material::parse_color(c)) {
                                Some(Ok(c)) => c,
                                Some(Err(e)) => {
                                    err(e);
                                    [255, 245, 225]
                                }
                                None => [255, 245, 225],
                            };
                            lighting.sun = Some(crate::light::Sun { angle: v[0].clamp(-75.0, 75.0), strength: v[1].clamp(0.0, 5.0), color });
                        }
                        Err(_) => err("use `sun ANGLE STRENGTH [#colour]` (angle from straight down: 30 = from the upper left)".into()),
                    }
                }
            }
            "light" | "lamp" => match nums(1, 3) {
                Ok(v) => {
                    let mut l = crate::light::Light::at(v[0], v[1], v[2].clamp(1.0, 400.0));
                    let mut i = 4;
                    let mut cone_world: Option<(f32, f32)> = None;
                    let mut bad: Option<String> = None;
                    while i < words.len() {
                        let num = |k: usize| words.get(k).and_then(|w| w.parse::<f32>().ok());
                        match words[i].to_lowercase().as_str() {
                            "strength" => {
                                l.strength = num(i + 1).unwrap_or(1.0).clamp(0.0, 10.0);
                                i += 2;
                            }
                            "color" | "colour" => {
                                match words.get(i + 1).map(|c| crate::material::parse_color(c)) {
                                    Some(Ok(c)) => l.color = c,
                                    _ => bad = Some("`color` needs a colour like #ffb060".into()),
                                }
                                i += 2;
                            }
                            "cone" => {
                                match (num(i + 1), num(i + 2)) {
                                    (Some(d), Some(w)) => cone_world = Some((d, w.clamp(1.0, 180.0))),
                                    _ => bad = Some("`cone` needs a direction and a width in degrees: `cone 0 25`".into()),
                                }
                                i += 3;
                            }
                            "flicker" => {
                                l.flicker = num(i + 1).unwrap_or(0.3).clamp(0.0, 1.0);
                                i += 2;
                            }
                            "on" => {
                                match words.get(i + 1).and_then(|w| names.get(&w.to_lowercase()).copied()) {
                                    Some(n) => l.on = Some(n as u64 + 1),
                                    None => bad = Some("`on` needs a named object (name it before the light)".into()),
                                }
                                i += 2;
                            }
                            other => {
                                bad = Some(format!("light: don't know `{other}`"));
                                i += 1;
                            }
                        }
                    }
                    // On an object: keep where it is relative to the object (and its turn).
                    let mut turn = 0.0f32;
                    if let Some(n) = l.on
                        && let Some(b) = bodies.get(n as usize - 1) {
                            let (dx, dy) = (l.x - b.x, l.y - b.y);
                            let (c, s) = (b.angle.cos(), b.angle.sin());
                            (l.x, l.y) = (c * dx + s * dy, -s * dx + c * dy);
                            turn = b.angle.to_degrees();
                        }
                    l.cone = cone_world.map(|(d, w)| (d - turn, w));
                    match bad {
                        Some(e) => err(e),
                        None => lighting.lights.push(l),
                    }
                }
                Err(_) => err("use `light X Y RADIUS [strength S] [color #c] [cone DIR WIDTH] [flicker F] [on NAME]`".into()),
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
            "slope" | "hill" | "curve" => {
                // Options at the end: `to Y` (bottom of the fill), `cover MAT DEPTH`.
                let opt_at = |k: &str| words.iter().position(|w| w.eq_ignore_ascii_case(k));
                let end = [opt_at("to"), opt_at("cover")].into_iter().flatten().min().unwrap_or(words.len());
                let need = match cmd.as_str() {
                    "slope" => 4,
                    "hill" => 4,
                    _ => 6,
                };
                let r = words.get(1).ok_or(format!("`{cmd}` needs a material")).and_then(|m| mat(m)).and_then(|id| {
                    if end < 2 + need {
                        return Err(format!("`{cmd}` needs {need} numbers after the material"));
                    }
                    nums(2, need).map(|v| (id, v))
                });
                let cover = opt_at("cover").and_then(|i| {
                    let m = words.get(i + 1).and_then(|w| mat(w).ok())?;
                    let d = words.get(i + 2).and_then(|w| w.parse::<f32>().ok())?;
                    Some((m, d.clamp(0.0, 200.0) as i32))
                });
                if opt_at("cover").is_some() && cover.is_none() {
                    err("use `cover MATERIAL DEPTH`".into());
                }
                match r {
                    Ok((id, v)) => {
                        let w = ensure(&mut world, mats);
                        let bottom = opt_at("to").and_then(|i| words.get(i + 1)).and_then(|y| y.parse::<f32>().ok()).unwrap_or(w.h as f32) as i32;
                        match cmd.as_str() {
                            "slope" => {
                                let (x1, y1, x2, y2) = (v[0], v[1], v[2], v[3]);
                                let top = move |x: f32| y1 + ((x - x1) * (y2 - y1) / (x2 - x1).max(1e-6)).round();
                                fill_under(w, mats, x1 as i32, x2 as i32, top, bottom, id, cover);
                            }
                            "hill" => {
                                let (x1, x2, height, base) = (v[0], v[1], v[2], v[3]);
                                let top = move |x: f32| base - (height * (std::f32::consts::PI * (x - x1) / (x2 - x1).max(1e-6)).sin().powi(2)).round();
                                fill_under(w, mats, x1 as i32, x2 as i32, top, bottom, id, cover);
                            }
                            _ => {
                                // A quadratic curve from (x1,y1) to (x2,y2), pulled towards (cx,cy).
                                let (p0, p1, p2) = ((v[0], v[1]), (v[2], v[3]), (v[4], v[5]));
                                let (lo, hi) = (p0.0.min(p2.0) as i32, p0.0.max(p2.0) as i32);
                                let mut tops = vec![f32::INFINITY; (hi - lo + 1).clamp(1, 20_000) as usize];
                                let steps = ((hi - lo).abs() * 8).clamp(8, 160_000);
                                for k in 0..=steps {
                                    let t = k as f32 / steps as f32;
                                    let q = |a: f32, b: f32, c: f32| (1.0 - t) * (1.0 - t) * a + 2.0 * (1.0 - t) * t * b + t * t * c;
                                    let (x, y) = (q(p0.0, p1.0, p2.0), q(p0.1, p1.1, p2.1));
                                    let col = (x.round() as i32 - lo).clamp(0, tops.len() as i32 - 1) as usize;
                                    tops[col] = tops[col].min(y.round());
                                }
                                let top = move |x: f32| tops[((x as i32 - lo).clamp(0, tops.len() as i32 - 1)) as usize];
                                fill_under(w, mats, lo, hi + 1, top, bottom, id, cover);
                            }
                        }
                    }
                    Err(e) => err(e),
                }
            }
            "poly" | "polygon" => {
                let r = words.get(1).ok_or("`poly` needs a material".to_string()).and_then(|m| mat(m)).and_then(|id| {
                    let n = words.len().saturating_sub(2);
                    if n < 6 || n % 2 == 1 {
                        return Err("use `poly MATERIAL x1 y1 x2 y2 x3 y3 ...` (three or more points)".into());
                    }
                    nums(2, n).map(|v| (id, v))
                });
                match r {
                    Ok((id, v)) => {
                        let pts: Vec<(f32, f32)> = v.chunks(2).map(|c| (c[0], c[1])).collect();
                        fill_poly(ensure(&mut world, mats), mats, &pts, id);
                    }
                    Err(e) => err(e),
                }
            }
            "trigger" => {
                let (head, actions) = line.split_once(':').unwrap_or((line, ""));
                let head: Vec<&str> = head.split_whitespace().collect();
                match crate::trigger::parse(&head, actions) {
                    Ok((mut t, by)) => {
                        t.by = match by.as_str() {
                            "any" => crate::trigger::By::Any,
                            "player" => crate::trigger::By::Player,
                            name => match names.get(name) {
                                Some(&n) => crate::trigger::By::Object(n as u64 + 1),
                                None => {
                                    err(format!("trigger: no object named `{name}` (name it before the trigger)"));
                                    continue;
                                }
                            },
                        };
                        triggers.push(t);
                    }
                    Err(e) => err(e),
                }
            }
            "anchor" => match nums(1, 4) {
                Ok(v) => {
                    let w = ensure(&mut world, mats);
                    let (x0, x1, y0, y1) = w.clip(v[0] as i32, (v[0] + v[2]) as i32 - 1, v[1] as i32, (v[1] + v[3]) as i32 - 1);
                    for y in y0..=y1 {
                        for x in x0..=x1 {
                            let i = w.idx(x as usize, y as usize);
                            if w.mat[i] != AIR {
                                w.flags[i] |= crate::world::ANCHORED;
                            }
                        }
                    }
                }
                Err(_) => err("use `anchor x y width height` (after drawing what it pins)".into()),
            },
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
                            (iv[0], iv[1], iv[0].saturating_add(iv[2]) - 1, iv[1].saturating_add(iv[3]) - 1, None)
                        } else {
                            let r = iv[2].clamp(0, 8192);
                            (iv[0] - r, iv[1] - r, iv[0] + r, iv[1] + r, Some((iv[0], iv[1], r)))
                        };
                        let (x0, x1, y0, y1) = w.clip(x0, x1, y0, y1);
                        for y in y0..=y1 {
                            for x in x0..=x1 {
                                if !w.in_bounds(x, y) {
                                    continue;
                                }
                                if let Some((cx, cy, r)) = circle
                                    && ((x - cx) as i64).pow(2) + ((y - cy) as i64).pow(2) > (r * r + r / 2) as i64 {
                                        continue;
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
                                let (w, h) = (v[2].clamp(1.0, 1024.0) as usize, v[3].clamp(1.0, 1024.0) as usize);
                                (w, h, vec![Some(id); w * h], v[0] + w as f32 / 2.0, v[1] + h as f32 / 2.0)
                            } else {
                                let r = v[2].clamp(0.0, 512.0) as i32;
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
            "depth" => {
                match words.get(1).and_then(|s|s.parse::<f32>().ok()) {
                    Some(d) if d.is_finite() && d > 0.0 && d <= 10.0 && !bodies.is_empty() => bodies.last_mut().unwrap().depth = d,
                    _ => err("depth needs a preceding body and a thickness in metres (0 < depth <= 10)".into()),
                }
            }
            "player" => {
                let opt = |key: &str| words.iter().position(|w| w.eq_ignore_ascii_case(key)).and_then(|i| words.get(i + 1));
                let material = opt("material").map(|s| s.to_string()).unwrap_or_else(|| "flesh".into());
                match (nums(1, 2), mat(&material)) {
                    (Ok(v), Ok(id)) => {
                        let mut a = crate::physics::ActorSpec::standing_at(id, v[0], v[1]);
                        if let Some(name) = opt("look") {
                            match crate::look::find(name, base, crate::physics::actor_height()) {
                                Ok(l) => a.look = Some(l),
                                Err(e) => err(e),
                            }
                        }
                        actors.push(a);
                        if let Some(z) = opt("zoom").and_then(|z| z.parse::<f32>().ok()) {
                            player_zoom = z.clamp(0.0, 20.0);
                        }
                    }
                    (Err(_), _) => err("use `player x y` (where its feet go), optionally `look hero material flesh zoom 3`".into()),
                    (_, Err(e)) => err(e),
                }
            }
            "name" => match (words.get(1), bodies.len()) {
                (Some(n), k) if k > 0 => {
                    names.insert(n.to_lowercase(), k - 1);
                }
                _ => err("`name` goes right after an object: `name crate1`".into()),
            },
            "upright" => match words.get(1).map(|w| w.to_lowercase()).and_then(|w| names.get(&w).copied()) {
                Some(n) => {
                    let val = |key: &str, default: f32| {
                        words
                            .iter()
                            .position(|w| w.eq_ignore_ascii_case(key))
                            .and_then(|i| words.get(i + 1))
                            .and_then(|z| z.parse::<f32>().ok())
                            .unwrap_or(default)
                    };
                    upright.push((n, crate::physics::Balance {
                        strength: val("strength", 60.0).clamp(0.0, 1000.0),
                        air: val("air", 1.0).clamp(0.0, 1.0),
                        heavy: val("heavy", 1.0).clamp(0.1, 5.0),
                    }));
                }
                None => err("use `upright NAME [strength 60] [air 1] [heavy 1]` with a named object".into()),
            },
            "follow" => match words.get(1).map(|w| w.to_lowercase()).and_then(|w| names.get(&w).copied()) {
                Some(n) => {
                    let zoom = words
                        .iter()
                        .position(|w| w.eq_ignore_ascii_case("zoom"))
                        .and_then(|i| words.get(i + 1))
                        .and_then(|z| z.parse::<f32>().ok())
                        .unwrap_or(0.0);
                    follow = Some((n, zoom.clamp(0.0, 20.0)));
                }
                None => err("use `follow NAME [zoom Z]` with a named object".into()),
            },
            "hinge" | "wheel" | "weld" | "rope" => {
                use crate::physics::{JointKind, Motor};
                let end = |w: Option<&&str>| -> Result<End, String> {
                    let w = w.ok_or(format!("`{cmd}` needs two names: `{cmd} first second`"))?.to_lowercase();
                    if w == "ground" {
                        return Ok(End::Ground);
                    }
                    names.get(&w).map(|&n| End::Object(n)).ok_or(format!("there's no object named `{w}` (use `name {w}` after it)"))
                };
                let (a, b) = match (end(words.get(1)), end(words.get(2))) {
                    (Ok(a), Ok(b)) => (a, b),
                    (Err(e), _) | (_, Err(e)) => {
                        err(e);
                        continue;
                    }
                };
                // The rest: keyword value pairs.
                let rest: Vec<String> = words.iter().skip(3).map(|w| w.to_lowercase()).collect();
                let val = |key: &str| -> Option<f32> { rest.iter().position(|w| w == key).and_then(|i| rest.get(i + 1)).and_then(|v| v.parse().ok()) };
                let pair = |key: &str| -> Option<(f32, f32)> {
                    let i = rest.iter().position(|w| w == key)?;
                    Some((rest.get(i + 1)?.parse().ok()?, rest.get(i + 2)?.parse().ok()?))
                };
                let kind = match cmd.as_str() {
                    "hinge" => JointKind::Hinge,
                    "wheel" => JointKind::Wheel {
                        stiffness: val("spring").unwrap_or(60.0),
                        damping: val("damping").unwrap_or(4.0),
                        travel: val("travel").unwrap_or(3.0),
                    },
                    "weld" => JointKind::Weld,
                    _ => JointKind::Rope { length: val("length").unwrap_or(0.0) },
                };
                let motor = val("motor").map(|speed| Motor {
                    speed,
                    strength: val("strength").unwrap_or(2.0),
                    drive: rest.iter().any(|w| w == "drive"),
                });
                joints.push(PendingJoint { a, b, kind, at: pair("at"), to: pair("to"), motor, break_force: val("break") });
            }
            "place" => {
                use crate::template;
                if !allow_place {
                    err("templates can't place other templates".into());
                    continue;
                }
                let Some(name) = words.get(1).map(|n| n.to_lowercase()) else {
                    err("use `place crate x y` (a template from the templates folder)".into());
                    continue;
                };
                let pos = match nums(2, 2) {
                    Ok(v) => v,
                    Err(e) => {
                        err(e);
                        continue;
                    }
                };
                let mut rest: Vec<&str> = words.iter().skip(4).copied().collect();
                let mut angle = 0.0f32;
                if rest.first().is_some_and(|w| w.eq_ignore_ascii_case("angle")) {
                    angle = rest.get(1).and_then(|a| a.parse().ok()).unwrap_or(0.0);
                    rest.drain(..rest.len().min(2));
                }
                let (changes, bad) = template::parse_changes(&rest);
                if !bad.is_empty() {
                    err(format!("didn't understand `{}` (use material NAME, flip, flipv, turn, scale N)", bad.join(" ")));
                }
                if !template_cache.contains_key(&name) {
                    match template::find(&name, base).ok_or(format!("there's no template called `{name}` in a templates folder")).and_then(|p| template::load_file(&p, mats)) {
                        Ok(t) => {
                            template_cache.insert(name.clone(), t);
                        }
                        Err(e) => {
                            err(e);
                            continue;
                        }
                    }
                }
                let t = &template_cache[&name];
                let mut spec = template::build(&t.shape, &changes, mats);
                spec.x = pos[0];
                spec.y = pos[1];
                spec.angle = angle.to_radians();
                spec.vx = 0.0;
                spec.vy = 0.0;
                spec.spin = 0.0;
                placed.push((bodies.len(), template::Instance { template: name, changes }));
                bodies.push(spec);
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
                // `material rock`: make the whole thing out of one material.
                let forced = rest.iter().position(|w| w == "material").and_then(|i| rest.get(i + 1)).map(|m| mat(m));
                if let Some(Err(e)) = &forced {
                    err(e.clone());
                }
                let forced = forced.and_then(|r| r.ok());
                match pos.and_then(|v| import::load(&path, mats, opt("scale")).map(|r| (v, r))) {
                    Ok((v, (mut shape, mut warnings))) => {
                        if let Some(id) = forced {
                            for c in shape.cells.iter_mut().flatten() {
                                *c = id;
                            }
                            // (Nothing to guess any more, so no colour warnings.)
                            warnings.retain(|w| !w.contains("colour") && !w.contains("material"));
                        }
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
    let mut instances = vec![None; bodies.len()];
    for (i, inst) in placed {
        instances[i] = Some(inst);
    }
    Loaded { world, bodies, problems, heated, instances, joints, follow, upright, triggers, lighting, backdrop, actors, player_zoom }
}

/// Fill each column from `top(x)` down to `bottom` (exclusive), for x in
/// x1..x2: the first `cover.1` cells with the cover material.
#[allow(clippy::too_many_arguments)]
fn fill_under(w: &mut World, mats: &Materials, x1: i32, x2: i32, top: impl Fn(f32) -> f32, bottom: i32, id: MatId, cover: Option<(MatId, i32)>) {
    let (a, b) = (x1.min(x2).max(0), x1.max(x2).min(w.w as i32));
    for x in a..b {
        let t = top(x as f32);
        if !t.is_finite() {
            continue;
        }
        let y0 = (t as i32).max(0);
        for y in y0..bottom.min(w.h as i32) {
            let m = match cover {
                Some((c, d)) if y < y0 + d => c,
                _ => id,
            };
            w.set(mats, x as usize, y as usize, m);
        }
    }
}

/// Fill a polygon (even-odd rule), cell centres inside.
fn fill_poly(w: &mut World, mats: &Materials, pts: &[(f32, f32)], id: MatId) {
    let ys = pts.iter().map(|p| p.1);
    let (lo, hi) = (ys.clone().fold(f32::MAX, f32::min), ys.fold(f32::MIN, f32::max));
    let (y0, y1) = ((lo.floor() as i32).max(0), (hi.ceil() as i32).min(w.h as i32 - 1));
    for y in y0..=y1 {
        let cy = y as f32 + 0.5;
        let mut xs: Vec<f32> = Vec::new();
        for k in 0..pts.len() {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            if (a.1 <= cy) != (b.1 <= cy) {
                xs.push(a.0 + (cy - a.1) / (b.1 - a.1) * (b.0 - a.0));
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks(2) {
            if let [a, b] = pair {
                let (xa, xb) = (((a - 0.5).ceil() as i32).max(0), ((b - 0.5).floor() as i32).min(w.w as i32 - 1));
                for x in xa..=xb {
                    w.set(mats, x as usize, y as usize, id);
                }
            }
        }
    }
}

fn rect(w: &mut World, mats: &Materials, x: i32, y: i32, rw: i32, rh: i32, id: MatId) {
    let (x0, x1, y0, y1) = w.clip(x, x.saturating_add(rw).saturating_sub(1), y, y.saturating_add(rh).saturating_sub(1));
    for yy in y0..=y1 {
        for xx in x0..=x1 {
            if w.in_bounds(xx, yy) {
                w.set(mats, xx as usize, yy as usize, id);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn line_shape(w: &mut World, mats: &Materials, x1: i32, y1: i32, x2: i32, y2: i32, thick: i32, id: MatId) {
    // Keep silly numbers from making a line billions of steps long.
    let lim = |v: i32| v.clamp(-16_384, 16_384);
    let (x1, y1, x2, y2) = (lim(x1), lim(y1), lim(x2), lim(y2));
    let steps = (x2 - x1).abs().max((y2 - y1).abs()).max(1);
    let r = (thick.clamp(1, 256) - 1) / 2;
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
    if world.wind != 0.0 {
        out.push_str(&format!("wind {}\n", world.wind));
    }
    if world.turbulence != 1.0 {
        out.push_str(&format!("turbulence {}\n", world.turbulence));
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
    // Anchored scenery: one line per run of anchored cells in a row.
    let mut anchors = String::new();
    for y in 0..world.h {
        let mut x = 0;
        while x < world.w {
            let anchored = |x: usize| world.flags[world.idx(x, y)] & crate::world::ANCHORED != 0;
            if anchored(x) {
                let start = x;
                while x < world.w && anchored(x) {
                    x += 1;
                }
                anchors += &format!("anchor {start} {y} {} 1\n", x - start);
            } else {
                x += 1;
            }
        }
    }
    if !anchors.is_empty() {
        out.push_str("# Pinned scenery (never collapses)\n");
        out.push_str(&anchors);
    }
    if !snap.bodies.is_empty() {
        out.push_str("\n# Objects: centre x y, angle in degrees, then their cells\n");
    }
    // Objects that joints refer to get names.
    let mut named: std::collections::HashSet<u64> = snap.joints.iter().flat_map(|j| [j.a, j.b]).collect();
    if let Some(f) = snap.follow {
        named.insert(f);
    }
    named.extend(snap.upright.iter().map(|&(n, _)| n));
    named.extend(snap.triggers.iter().filter_map(|t| match t.by {
        crate::trigger::By::Object(n) => Some(n),
        _ => None,
    }));
    named.extend(snap.lighting.lights.iter().filter_map(|l| l.on));
    let name_line = |n: usize| if named.contains(&(n as u64 + 1)) { format!("name obj{}\n", n + 1) } else { String::new() };
    for (n, b) in snap.bodies.iter().enumerate() {
        if let Some(Some(inst)) = snap.instances.get(n) {
            // Still exactly its template (plus changes)? Save it as one line.
            let fits = crate::template::find(&inst.template, None)
                .and_then(|p| crate::template::load_file(&p, mats).ok())
                .is_some_and(|t| crate::template::matches(b, &crate::template::build(&t.shape, &inst.changes, mats)));
            if fits {
                let changes: Vec<String> = inst.changes.iter().map(|c| c.to_words()).collect();
                out.push_str(&format!(
                    "place {} {:.2} {:.2} angle {:.1} {}\n",
                    inst.template,
                    b.x,
                    b.y,
                    b.angle.to_degrees(),
                    changes.join(" ")
                ));
                if b.depth != 1.0 {out.push_str(&format!("depth {}\n",b.depth));}
                out.push_str(&name_line(n));
                continue;
            }
        }
        out.push_str(&format!("body {:.2} {:.2} {:.1}\n", b.x, b.y, b.angle.to_degrees()));
        for y in 0..b.h {
            for x in 0..b.w {
                out.push(b.cells[y * b.w + x].map(|c| chars[&c.mat]).unwrap_or('.'));
            }
            out.push('\n');
        }
        out.push_str("end\n");
        if b.depth != 1.0 { out.push_str(&format!("depth {}\n", b.depth)); }
        out.push_str(&name_line(n));
    }
    if !snap.joints.is_empty() {
        out.push_str("\n# Joints\n");
    }
    let world_at = |id: u64, p: (f32, f32)| -> (f32, f32) {
        if id == crate::physics::GROUND {
            return p;
        }
        let b = &snap.bodies[id as usize - 1];
        let (c, s) = (b.angle.cos(), b.angle.sin());
        (b.x + c * p.0 - s * p.1, b.y + s * p.0 + c * p.1)
    };
    let who = |id: u64| if id == crate::physics::GROUND { "ground".to_string() } else { format!("obj{id}") };
    for j in &snap.joints {
        use crate::physics::JointKind;
        let at = world_at(j.b, j.anchor_b);
        let mut line = match &j.kind {
            JointKind::Hinge => format!("hinge {} {} at {:.2} {:.2}", who(j.a), who(j.b), at.0, at.1),
            JointKind::Wheel { stiffness, damping, travel } => format!(
                "wheel {} {} at {:.2} {:.2} spring {stiffness} damping {damping} travel {travel}",
                who(j.a),
                who(j.b),
                at.0,
                at.1
            ),
            JointKind::Weld => format!("weld {} {} at {:.2} {:.2}", who(j.a), who(j.b), at.0, at.1),
            JointKind::Rope { length } => {
                let from = world_at(j.a, j.anchor_a);
                format!("rope {} {} at {:.2} {:.2} to {:.2} {:.2} length {length:.2}", who(j.a), who(j.b), from.0, from.1, at.0, at.1)
            }
        };
        if let Some(m) = &j.motor {
            line += &format!(" motor {} strength {}{}", m.speed, m.strength, if m.drive { " drive" } else { "" });
        }
        if let Some(force) = j.break_force { line += &format!(" break {force}"); }
        out.push_str(&line);
        out.push('\n');
    }
    if let Some(f) = snap.follow {
        out.push_str(&format!("\nfollow obj{f} zoom {}\n", snap.zoom));
    }
    for &(n, b) in &snap.upright {
        out.push_str(&format!("upright obj{n} strength {}", b.strength));
        if b.air < 1.0 {
            out.push_str(&format!(" air {}", b.air));
        }
        if b.heavy != 1.0 {
            out.push_str(&format!(" heavy {}", b.heavy));
        }
        out.push('\n');
    }
    for t in &snap.triggers {
        let by = match t.by {
            crate::trigger::By::Any => "any".to_string(),
            crate::trigger::By::Player => "player".to_string(),
            crate::trigger::By::Object(n) => format!("obj{n}"),
        };
        out.push_str(&t.to_line(&by));
        out.push('\n');
    }
    if let Some((x, y)) = snap.checkpoint {
        out.push_str(&format!("# Last checkpoint reached: {x} {y}\n"));
    }
    out.push_str(&snap.backdrop.to_lines());
    out.push_str(&snap.lighting.to_lines(|n| format!("obj{n}"), |n| snap.bodies.get(n as usize - 1).map(|b| (b.x, b.y, b.angle))));
    for a in &snap.actors {
        let (x, y) = a.feet();
        let mut line = format!("\nplayer {x:.1} {y:.1}");
        if let Some(l) = &a.look {
            // Its name if that finds it again (assets/characters/NAME), else its folder.
            let same = |p: &Path| p.canonicalize().ok() == l.dir.canonicalize().ok();
            let by_name = crate::look::find(&l.name, None, crate::physics::actor_height()).is_ok_and(|f| same(&f.dir));
            let r = if by_name { l.name.clone() } else { l.dir.display().to_string().replace('\\', "/") };
            line += &format!(" look {r}");
        }
        if mats.get(a.mat).name != "flesh" {
            line += &format!(" material {}", mats.get(a.mat).name);
        }
        if snap.follow.is_none() && snap.zoom > 0.0 {
            line += &format!(" zoom {}", snap.zoom);
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}
