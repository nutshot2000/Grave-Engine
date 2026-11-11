//! Weather: snow that builds up, rain, hail, sandstorms, fog, gusty wind and
//! lightning that sets things on fire.
//!
//! ```text
//! weather snow 0.6          # how heavy, 0 (none) to 1 (blizzard)
//! weather rain 0.4
//! weather hail 0.3
//! weather dust 0.5          # a sandstorm (also: sandstorm)
//! weather fog 0.5
//! weather gusts 0.6         # the wind rises and falls in waves (on top of `wind`)
//! weather lightning 6       # strikes per minute (0 = none)
//! weather off               # switch all weather off (the settings stay)
//! ```
//!
//! - Everything is real material: snow, rain, hail and dust are flying cells
//!   that land and pile up, so snow drifts deepen, rain fills puddles and puts
//!   fires out, hail melts, tyres bog down in the loose stuff.
//! - It falls around the camera (`Sim::weather_view`, set by the editor and
//!   games each frame; otherwise around the followed object, or over the whole
//!   world), so a storm costs the same however big the level is.
//! - **Lightning** picks the tallest thing in view, flickers a warning, then
//!   strikes: a flash, a shake, a blast that heats and cracks, fire on
//!   anything flammable, sand fused to glass. It lights the scene up.
//! - Triggers change it over time: `weather snow 0.5 6` (to 0.5 over 6 s),
//!   `weather clear 3` (everything fades out).

use crate::camera::Effects;
use crate::material::{Materials, State};
use crate::world::{Blast, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Snow,
    Rain,
    Hail,
    Dust,
    Fog,
    Lightning,
    Gusts,
}

impl Kind {
    pub const ALL: [Kind; 7] = [Kind::Snow, Kind::Rain, Kind::Hail, Kind::Dust, Kind::Fog, Kind::Gusts, Kind::Lightning];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Snow => "snow",
            Kind::Rain => "rain",
            Kind::Hail => "hail",
            Kind::Dust => "dust",
            Kind::Fog => "fog",
            Kind::Lightning => "lightning",
            Kind::Gusts => "gusts",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s.to_lowercase().as_str() {
            "snow" => Kind::Snow,
            "rain" => Kind::Rain,
            "hail" => Kind::Hail,
            "dust" | "sandstorm" | "sand" => Kind::Dust,
            "fog" | "mist" => Kind::Fog,
            "lightning" | "thunder" => Kind::Lightning,
            "gusts" | "gusty" | "gust" => Kind::Gusts,
            _ => return None,
        })
    }

    /// The biggest value (1 for how-heavy settings, strikes per minute for lightning).
    pub fn max(self) -> f32 {
        if self == Kind::Lightning { 30.0 } else { 1.0 }
    }
}

/// The weather settings (saved with the scene).
#[derive(Clone, Debug, PartialEq)]
pub struct Weather {
    /// Master switch: off, nothing falls and nothing strikes (the settings stay).
    pub on: bool,
    pub snow: f32,
    pub rain: f32,
    pub hail: f32,
    pub dust: f32,
    pub fog: f32,
    pub gusts: f32,
    /// Strikes per minute.
    pub lightning: f32,
    /// Changes under way: (what, to, frames left).
    pub fades: Vec<(Kind, f32, u32)>,
    /// The scene's steady wind, remembered while gusts blow on top of it.
    pub gust_base: Option<f32>,
}

impl Default for Weather {
    fn default() -> Self {
        Weather { on: true, snow: 0.0, rain: 0.0, hail: 0.0, dust: 0.0, fog: 0.0, gusts: 0.0, lightning: 0.0, fades: Vec::new(), gust_base: None }
    }
}

/// What's happening right now (not saved): the flash, the bolt, the timer.
#[derive(Clone, Debug, Default)]
pub struct WeatherRun {
    /// How bright the sky flash is, 1 → 0.
    pub flash: f32,
    /// The bolt's cells, and how many frames it stays.
    pub bolt: Vec<(i32, i32)>,
    /// How many of the bolt's cells are the main trunk (the rest are thinner forks).
    pub bolt_main: usize,
    pub bolt_frames: u32,
    /// Frames until the next strike is planned.
    pub timer: i32,
    /// A strike on its way: (x, y, frames of warning left).
    pub pending: Option<(f32, f32, u32)>,
    /// A strike called for now (the editor's button).
    pub strike_now: bool,
}

impl Weather {
    pub fn get(&self, k: Kind) -> f32 {
        match k {
            Kind::Snow => self.snow,
            Kind::Rain => self.rain,
            Kind::Hail => self.hail,
            Kind::Dust => self.dust,
            Kind::Fog => self.fog,
            Kind::Lightning => self.lightning,
            Kind::Gusts => self.gusts,
        }
    }

    fn put(&mut self, k: Kind, v: f32) {
        let v = v.clamp(0.0, k.max());
        match k {
            Kind::Snow => self.snow = v,
            Kind::Rain => self.rain = v,
            Kind::Hail => self.hail = v,
            Kind::Dust => self.dust = v,
            Kind::Fog => self.fog = v,
            Kind::Lightning => self.lightning = v,
            Kind::Gusts => self.gusts = v,
        }
    }

    /// Change one setting, at once (`seconds` = 0) or fading over `seconds`.
    pub fn set(&mut self, k: Kind, to: f32, seconds: f32) {
        self.fades.retain(|f| f.0 != k);
        let frames = (seconds * 60.0).round() as u32;
        if frames == 0 {
            self.put(k, to);
        } else {
            self.fades.push((k, to.clamp(0.0, k.max()), frames));
        }
    }

    /// Fade everything out.
    pub fn clear(&mut self, seconds: f32) {
        for k in Kind::ALL {
            self.set(k, 0.0, seconds);
        }
    }

    /// Nothing falling, blowing or striking.
    pub fn is_clear(&self) -> bool {
        Kind::ALL.iter().all(|&k| self.get(k) <= 0.0) && self.fades.is_empty()
    }

    /// Ready-made weather: (name, settings).
    pub fn presets() -> Vec<(&'static str, Weather)> {
        let w = |snow: f32, rain: f32, hail: f32, dust: f32, fog: f32, gusts: f32, lightning: f32| Weather { snow, rain, hail, dust, fog, gusts, lightning, ..Weather::default() };
        vec![
            ("☀ Clear", Weather::default()),
            ("❄ Snowfall", w(0.35, 0.0, 0.0, 0.0, 0.1, 0.0, 0.0)),
            ("🌨 Blizzard", w(1.0, 0.0, 0.0, 0.0, 0.45, 0.8, 0.0)),
            ("🌧 Rain", w(0.0, 0.5, 0.0, 0.0, 0.15, 0.0, 0.0)),
            ("⛈ Storm", w(0.0, 0.8, 0.0, 0.0, 0.25, 0.7, 6.0)),
            ("🧊 Hailstorm", w(0.0, 0.2, 0.6, 0.0, 0.15, 0.4, 2.0)),
            ("🏜 Sandstorm", w(0.0, 0.0, 0.0, 0.7, 0.35, 0.8, 0.0)),
            ("🌫 Fog", w(0.0, 0.0, 0.0, 0.0, 0.7, 0.0, 0.0)),
        ]
    }

    /// Read one scene line. Ok(false) = not a weather line.
    pub fn parse_line(&mut self, words: &[&str]) -> Result<bool, String> {
        if !words.first().is_some_and(|w| w.eq_ignore_ascii_case("weather")) {
            return Ok(false);
        }
        let usage = "use `weather snow|rain|hail|dust|fog|gusts 0.5`, `weather lightning 6` (strikes a minute), `weather off`";
        match words.get(1).map(|w| w.to_lowercase()).as_deref() {
            Some("off") => self.on = false,
            Some("on") => self.on = true,
            Some("clear") | Some("none") => {
                for k in Kind::ALL {
                    self.put(k, 0.0);
                }
            }
            Some(other) => {
                let k = Kind::parse(other).ok_or(format!("weather: don't know `{other}` ({usage})"))?;
                let v = words.get(2).and_then(|v| v.parse::<f32>().ok()).ok_or(usage.to_string())?;
                self.put(k, v);
            }
            None => return Err(usage.to_string()),
        }
        Ok(true)
    }

    /// The scene lines.
    pub fn to_lines(&self) -> String {
        let mut s = String::new();
        for k in Kind::ALL {
            let v = self.fades.iter().find(|f| f.0 == k).map_or(self.get(k), |f| f.1); // saved as where it's heading
            if v > 0.0 {
                s += &format!("weather {} {}\n", k.name(), (v * 1000.0).round() / 1000.0);
            }
        }
        if !self.on {
            s += "weather off\n";
        }
        if s.is_empty() { s } else { format!("\n# Weather\n{s}") }
    }
}

fn hash(a: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA77);
    x ^= x >> 13;
    x
}

/// Snow deeper than this many cells packs down.
const SNOW_DEPTH: usize = 16;

/// A random 0..1 from a few numbers.
fn rnd(frame: u32, a: u32, b: u32) -> f32 {
    hash(frame.wrapping_mul(2654435761) ^ a.wrapping_mul(40503) ^ b.wrapping_mul(7919)) as f32 / u32::MAX as f32
}

/// Where weather falls this frame: (x0, y0, x1, y1) in cells.
fn area(world: &World, view: Option<(f32, f32, f32, f32)>, follow: Option<(f32, f32)>) -> (f32, f32, f32, f32) {
    let (w, h) = (world.w as f32, world.h as f32);
    let (x0, y0, x1, y1) = match (view, follow) {
        (Some(v), _) => v,
        (None, Some((x, y))) => (x - 330.0, y - 200.0, x + 330.0, y + 200.0),
        _ => (0.0, 0.0, w, h),
    };
    (x0.max(0.0), y0.max(0.0), x1.min(w), y1.min(h))
}

/// How many things to make this frame from a rate (fractions come up now and then).
fn count(rate: f32, frame: u32, salt: u32) -> u32 {
    let n = rate.floor();
    n as u32 + (rnd(frame, salt, 99) < rate - n) as u32
}

/// One frame of weather: changes, falling stuff, wind gusts, lightning.
/// Call right after `World::step` (strikes add blasts for the physics to use).
#[allow(clippy::too_many_arguments)]
pub fn step(
    world: &mut World,
    mats: &Materials,
    w: &mut Weather,
    run: &mut WeatherRun,
    view: Option<(f32, f32, f32, f32)>,
    follow: Option<(f32, f32)>,
    camera: &mut Effects,
) {
    // Fades.
    let fades = std::mem::take(&mut w.fades);
    for (k, to, left) in fades {
        let v = w.get(k);
        w.put(k, v + (to - v) / left.max(1) as f32);
        if left > 1 {
            w.fades.push((k, to, left - 1));
        }
    }
    // Gusts: the wind rises and falls in waves on top of the scene's wind.
    if w.on && w.gusts > 0.0 {
        let base = *w.gust_base.get_or_insert(world.wind);
        let t = world.frame as f32;
        let wave = (t / 90.0).sin() + 0.6 * (t / 31.0 + 1.3).sin() + 0.3 * (t / 197.0).sin();
        world.wind = (base + w.gusts * 5.0 * wave).clamp(-50.0, 50.0);
    } else if let Some(base) = w.gust_base.take() {
        world.wind = base;
    }
    // The flash and the bolt fade.
    run.flash = if run.flash < 0.02 { 0.0 } else { run.flash * 0.84 };
    run.bolt_frames = run.bolt_frames.saturating_sub(1);
    if run.bolt_frames == 0 {
        run.bolt.clear();
    }
    if !w.on {
        run.pending = None;
        return;
    }

    let frame = world.frame;
    let (x0, y0, x1, y1) = area(world, view, follow);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let width = x1 - x0;
    let room = world.flying.len() < crate::world::MAX_FLYING * 7 / 10;
    let wind_cells = world.wind * 10.0 / 60.0; // cells per frame
    let ambient = world.ambient;
    let id = |name: &str| mats.id(name);

    // A spawn needs open air at its start.
    let open_at = |world: &World, x: f32, y: f32| -> bool {
        if x < 0.0 || y < 0.0 || x >= world.w as f32 || y >= world.h as f32 {
            return y < 0.0 && x >= 0.0 && x < world.w as f32;
        }
        let i = world.idx(x as usize, y as usize);
        !world.is_owned(i) && mats.get(world.mat[i]).state == State::Gas
    };

    if room {
        let spawn = |world: &mut World, mat: Option<crate::material::MatId>, x: f32, y: f32, vx: f32, vy: f32, cold: Option<f32>| {
            let Some(mat) = mat else { return };
            if !open_at(world, x, y) {
                return;
            }
            if world.spawn_flyer(mats, mat, x, y, vx, vy) {
                if let (Some(f), Some(t)) = (world.flying.last_mut(), cold) {
                    f.temp = t.min(ambient.min(f.temp)).max(-60.0);
                }
            }
        };
        let top = |k: u32| (y0 - 4.0 - rnd(frame, k, 5) * 12.0).max(0.5);
        // Snow: slow, carried sideways by the wind.
        for k in 0..count(w.snow * width * 0.008, frame, 1) {
            let x = x0 + rnd(frame, k, 1) * width - wind_cells * 60.0 * 0.25;
            spawn(world, id("snow"), x, top(k), wind_cells * 0.3, 0.1 + 0.3 * rnd(frame, k, 8), Some(-5.0));
        }
        // Rain: fast, at a slant.
        for k in 0..count(w.rain * width * 0.03, frame, 2) {
            let x = x0 + rnd(frame, k, 2) * width - wind_cells * 25.0;
            spawn(world, id("water"), x, top(k), wind_cells * 0.5, 3.0, Some(ambient.min(12.0)));
        }
        // Hail: fast and heavy.
        for k in 0..count(w.hail * width * 0.006, frame, 3) {
            let x = x0 + rnd(frame, k, 3) * width - wind_cells * 20.0;
            spawn(world, id("hail"), x, top(k), wind_cells * 0.4, 3.5, Some(-2.0));
        }
        // Dust: blows in from upwind along the ground.
        let dir = if world.wind < 0.0 { -1.0 } else { 1.0 };
        for k in 0..count(w.dust * (y1 - y0) * 0.05, frame, 4) {
            let (x, y) = (if dir > 0.0 { (x0 - 3.0).max(0.5) } else { (x1 + 3.0).min(world.w as f32 - 1.5) }, y0 + rnd(frame, k, 4) * (y1 - y0));
            spawn(world, id("dust"), x, y, dir * (1.5 + wind_cells.abs() * 0.5), 0.0, None);
        }
    }

    // Snow packs down under its own weight: deep drifts squash, so a long
    // snowfall settles into a believable layer instead of a mountain.
    if w.snow > 0.0 {
        if let Some(snow) = id("snow") {
            for k in 0..60u32 {
                let x = (x0 + rnd(frame, k, 21) * width) as usize;
                let x = x.min(world.w - 1);
                let mut y = y0 as usize;
                while y < world.h {
                    let i = world.idx(x, y);
                    if world.is_owned(i) || mats.get(world.mat[i]).state != State::Gas {
                        break;
                    }
                    y += 1;
                }
                let mut run = 0;
                while y + run < world.h && run < 40 {
                    let i = world.idx(x, y + run);
                    if world.mat[i] != snow || world.is_owned(i) {
                        break;
                    }
                    run += 1;
                }
                if run > SNOW_DEPTH && rnd(frame, k, 22) < 0.5 {
                    let i = world.idx(x, y + run - 1 - SNOW_DEPTH / 3);
                    world.wake_at(i);
                    world.transform(mats, i, crate::material::AIR);
                }
            }
        }
    }

    // Lightning.
    if w.lightning > 0.0 {
        run.timer -= 1;
        if run.timer <= 0 && run.pending.is_none() {
            run.strike_now = true;
            run.timer = (3600.0 / w.lightning * (0.4 + 1.2 * rnd(frame, 7, 7))) as i32;
        }
    } else {
        run.timer = 0;
    }
    if run.strike_now && run.pending.is_none() {
        run.strike_now = false;
        // Lightning likes what sticks up (the tallest, mostly) but not always:
        // about a third of strikes land on open ground, or on something lower.
        let mut tops: Vec<(f32, f32)> = Vec::new();
        for k in 0..32u32 {
            let x = (x0 + rnd(frame, k, 11) * width).min(world.w as f32 - 1.0);
            let col = x as usize;
            let top_y = (0..world.h).find(|&y| {
                let i = world.idx(col, y);
                world.is_owned(i) || mats.get(world.mat[i]).state != State::Gas
            });
            if let Some(y) = top_y {
                tops.push((x, y as f32));
            }
        }
        let mut ys: Vec<f32> = tops.iter().map(|t| t.1).collect();
        ys.sort_by(|a, b| a.total_cmp(b));
        let ground = ys.get(ys.len() * 3 / 4).copied().unwrap_or(0.0);
        let mut raised: Vec<(f32, f32)> = tops.iter().copied().filter(|t| t.1 < ground - 6.0).collect();
        raised.sort_by(|a, b| a.1.total_cmp(&b.1));
        let best = if raised.is_empty() || rnd(frame, 5, 12) < 0.3 {
            tops.get((rnd(frame, 6, 12) * tops.len() as f32) as usize % tops.len().max(1)).copied()
        } else if rnd(frame, 3, 12) < 0.7 {
            raised.first().copied()
        } else {
            Some(raised[(rnd(frame, 4, 12) * raised.len() as f32) as usize % raised.len()])
        };
        if let Some((x, y)) = best {
            run.pending = Some((x, y, 26)); // a flicker of warning first
        }
    }
    if let Some((x, y, left)) = run.pending {
        if left == 14 || left == 7 {
            run.flash = run.flash.max(0.22);
        }
        if left > 1 {
            run.pending = Some((x, y, left - 1));
        } else {
            run.pending = None;
            strike(world, mats, run, camera, x, y);
        }
    }
}

/// A bolt hits the top of the world at (x, y).
fn strike(world: &mut World, mats: &Materials, run: &mut WeatherRun, camera: &mut Effects, x: f32, y: f32) {
    let frame = world.frame;
    // The bolt: a jagged trunk down from the sky ending exactly at the strike,
    // with forks branching off it. Every strike is shaped from scratch (how
    // jagged, how many forks, which way they lean), so none looks the same.
    let mut n = 0u32;
    let mut r = |lo: f32, hi: f32| {
        n += 1;
        lo + (hi - lo) * rnd(frame, n, 31)
    };
    let height = r(220.0, 340.0);
    let from = (y - height).max(0.0);
    let start = (x + r(-40.0, 40.0), from);
    let jagged = r(0.18, 0.4);
    let mut trunk = vec![start, (x, y)];
    // Split each segment at its middle and push the middle sideways, again and again.
    let mut disp = (y - from) * jagged;
    for _ in 0..6 {
        let mut next = Vec::with_capacity(trunk.len() * 2);
        for pair in trunk.windows(2) {
            next.push(pair[0]);
            let mid = ((pair[0].0 + pair[1].0) * 0.5 + r(-1.0, 1.0) * disp * 0.5, (pair[0].1 + pair[1].1) * 0.5);
            next.push(mid);
        }
        next.push(*trunk.last().unwrap());
        trunk = next;
        disp *= 0.55;
    }
    let draw = |pts: &[(f32, f32)], out: &mut Vec<(i32, i32)>| {
        for pair in pts.windows(2) {
            let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
            let steps = (bx - ax).abs().max((by - ay).abs()).ceil().max(1.0) as i32;
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                out.push(((ax + (bx - ax) * t) as i32, (ay + (by - ay) * t) as i32));
            }
        }
    };
    run.bolt.clear();
    draw(&trunk, &mut run.bolt);
    run.bolt_main = run.bolt.len();
    // Forks: thinner branches that leave the trunk heading down and out, some forking again.
    let forks = r(2.0, 7.0) as usize;
    for _ in 0..forks {
        let at = (r(0.1, 0.9) * (trunk.len() - 1) as f32) as usize;
        let side = if r(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
        let mut stack = vec![(trunk[at], side * r(0.35, 1.4), r(30.0, 90.0), 0)];
        while let Some((p, lean, len, depth)) = stack.pop() {
            let mut pts = vec![p];
            let (mut cx, mut cy) = p;
            let steps = (len / 4.0) as usize;
            for s in 0..steps {
                cx += lean * 3.0 + r(-3.0, 3.0);
                cy += 3.0 + r(0.0, 2.5);
                if cy >= y - 2.0 {
                    break;
                }
                pts.push((cx, cy));
                if depth < 1 && s > 2 && s + 3 < steps && r(0.0, 1.0) < 0.1 {
                    stack.push(((cx, cy), -lean * r(0.5, 1.2), len * r(0.25, 0.5), depth + 1));
                }
            }
            if pts.len() > 1 {
                draw(&pts, &mut run.bolt);
            }
        }
    }
    run.bolt_frames = 16;
    run.flash = 1.0;
    camera.shake(5.0);
    let (cx, cy) = (x as i32, y as i32);
    // Sand fused to glass where it struck.
    if let Some(glass) = mats.id("glass") {
        let sandy = ["sand", "wet_sand", "sandstone", "dust"].iter().filter_map(|n| mats.id(n)).collect::<Vec<_>>();
        let (x0, x1, y0, y1) = world.clip(cx - 3, cx + 3, cy - 3, cy + 3);
        for yy in y0..=y1 {
            for xx in x0..=x1 {
                let i = world.idx(xx as usize, yy as usize);
                if !world.is_owned(i) && sandy.contains(&world.mat[i]) && (xx - cx).pow(2) + (yy - cy).pow(2) <= 10 {
                    world.transform(mats, i, glass);
                }
            }
        }
    }
    // Heat enough to light anything flammable, and a blast that cracks and throws.
    world.heat_circle(cx, cy, 3, 2500.0);
    world.heat_circle(cx, cy - 1, 7, 700.0);
    world.blasts.push(Blast { x: x + 0.5, y: y - 0.5, size: 6.0, power: 7000.0, fire: mats.id("fire") });
    world.wake_box(cx - 12, cy - 12, cx + 12, cy + 12);
}
