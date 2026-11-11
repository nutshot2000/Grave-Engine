//! The camera: what the player sees, shared by the editor and games.
//!
//! Two parts:
//!
//! - **Effects** (`Sim::camera`): things the level or game asks for, each for
//!   a while: slow motion, zooming in, looking at something, shaking. Level
//!   triggers set them without code (see trigger.rs):
//!
//!   ```text
//!   trigger 1490 0 30 260 by frame : slowmo 0.3 1.5 ; zoom 2 1.5 ; focus it
//!   trigger 2650 0 10 260 by frame : shake 4 ; message LAVA!
//!   ```
//!
//!   Games can call the same things (`sim.camera.slow_motion(0.3, 1.5)`).
//!   Timings are in real seconds as the player feels them (slow motion
//!   included), counted in simulation frames so they replay exactly.
//!
//! - **Camera** (one per window): eases towards where it should look, leads
//!   ahead of fast movers, adds shake. `Camera::update` takes what the game
//!   or editor wants by default and applies the effects on top.

use crate::sim::Sim;

/// Something to look at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Focus {
    /// A point (cells).
    Point(f32, f32),
    /// An object (its id while running).
    Object(u64),
    /// A character (the player): index into the characters.
    Actor(usize),
}

/// A value that lasts a number of simulation frames (`u32::MAX` = until
/// changed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timed<T> {
    pub value: T,
    pub frames: u32,
}

/// Camera effects asked for by the level or the game.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effects {
    /// How fast time runs (0.3 = 30 %).
    pub slowmo: Option<Timed<f32>>,
    /// How close the camera is, times closer than its usual view (2 = twice as close).
    pub zoom: Option<Timed<f32>>,
    /// What the camera looks at instead of its usual target.
    pub focus: Option<Timed<Focus>>,
    /// Shaking, in cells (dies away by itself).
    pub shake: f32,
    /// Frames run (for the shake pattern).
    pub tick: u32,
}

/// Seconds (as the player feels them) into simulation frames, at a time scale.
fn frames(seconds: f32, scale: f32) -> u32 {
    if seconds <= 0.0 || !seconds.is_finite() {
        u32::MAX
    } else {
        (seconds * 60.0 * scale).round().max(1.0) as u32
    }
}

impl Effects {
    /// Slow time down to `scale` (0.05..1) for `seconds` (0 = until changed).
    pub fn slow_motion(&mut self, scale: f32, seconds: f32) {
        let scale = scale.clamp(0.05, 1.0);
        self.slowmo = (scale < 1.0).then(|| Timed { value: scale, frames: frames(seconds, scale) });
    }

    /// Zoom in `times` closer than usual for `seconds` (0 = until changed).
    pub fn zoom_in(&mut self, times: f32, seconds: f32) {
        let times = times.clamp(0.1, 20.0);
        self.zoom = Some(Timed { value: times, frames: frames(seconds, self.time_scale()) });
    }

    /// Look at something for `seconds` (0 = until changed).
    pub fn look_at(&mut self, focus: Focus, seconds: f32) {
        self.focus = Some(Timed { value: focus, frames: frames(seconds, self.time_scale()) });
    }

    /// Shake the view (cells). Stronger shakes add up, to a limit.
    pub fn shake(&mut self, cells: f32) {
        self.shake = (self.shake + cells.max(0.0)).min(20.0);
    }

    /// Back to normal.
    pub fn reset(&mut self) {
        *self = Effects { tick: self.tick, ..Default::default() };
    }

    /// How fast time runs now (1 = normal).
    pub fn time_scale(&self) -> f32 {
        self.slowmo.map_or(1.0, |s| s.value)
    }

    /// One simulation frame passed.
    pub fn step(&mut self) {
        fn tick<T>(t: &mut Option<Timed<T>>) {
            if let Some(v) = t
                && v.frames != u32::MAX {
                    v.frames = v.frames.saturating_sub(1);
                    if v.frames == 0 {
                        *t = None;
                    }
                }
        }
        tick(&mut self.slowmo);
        tick(&mut self.zoom);
        tick(&mut self.focus);
        // Shake dies away over about half a second (of the player's time).
        self.shake *= 1.0 - 0.1 * self.time_scale().max(0.2);
        if self.shake < 0.05 {
            self.shake = 0.0;
        }
        self.tick = self.tick.wrapping_add(1);
    }

    /// How far to nudge the view this frame for the shake (cells): a jittery
    /// pattern that doesn't repeat quickly.
    pub fn shake_offset(&self) -> (f32, f32) {
        if self.shake <= 0.0 {
            return (0.0, 0.0);
        }
        let t = self.tick as f32;
        (self.shake * ((t * 2.31).sin() * 0.6 + (t * 5.17).sin() * 0.4), self.shake * ((t * 3.07).cos() * 0.6 + (t * 4.43).sin() * 0.4))
    }

    /// Where the focus is now, if there is one (and it still exists).
    pub fn focus_point(&self, sim: &Sim) -> Option<(f32, f32)> {
        match self.focus?.value {
            Focus::Point(x, y) => Some((x, y)),
            Focus::Object(id) => sim.physics.centre(id),
            Focus::Actor(n) => sim.physics.actor_status().get(n).map(|a| (a.x, a.y)),
        }
    }
}

/// What the view should be: centre (cells), zoom (times closer than the
/// whole-world view, or whatever "1" means to the caller), and a shake offset
/// (cells) to add when drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
    pub shake: (f32, f32),
}

/// A smoothly moving camera.
#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
    /// How quickly it catches up (per second): 8 is snappy, 2 is lazy.
    pub smooth: f32,
    /// How far ahead of a moving target it looks, in seconds of its movement.
    pub lead: f32,
    started: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { x: 0.0, y: 0.0, zoom: 1.0, smooth: 6.0, lead: 0.25, started: false }
    }
}

impl Camera {
    /// Jump straight to a view (no easing).
    pub fn snap(&mut self, x: f32, y: f32, zoom: f32) {
        (self.x, self.y, self.zoom, self.started) = (x, y, zoom, true);
    }

    /// Move towards where it should look. `want` = the usual target (x, y,
    /// zoom) and its velocity (cells/s, for looking ahead); the sim's effects
    /// go on top. `dt` = real seconds since the last frame.
    pub fn update(&mut self, sim: &Sim, want: (f32, f32, f32), velocity: (f32, f32), dt: f32) -> View {
        let fx = &sim.camera;
        let (mut tx, mut ty) = (want.0 + velocity.0 * self.lead, want.1 + velocity.1 * self.lead * 0.5);
        if let Some((x, y)) = fx.focus_point(sim) {
            (tx, ty) = (x, y);
        }
        let tz = want.2 * fx.zoom.map_or(1.0, |z| z.value);
        if !self.started {
            self.snap(tx, ty, tz);
        }
        let dt = dt.clamp(0.0, 0.25);
        let k = 1.0 - (-dt * self.smooth.max(0.1)).exp();
        let kz = 1.0 - (-dt * self.smooth.max(0.1) * 0.6).exp();
        self.x += (tx - self.x) * k;
        self.y += (ty - self.y) * k;
        self.zoom += (tz - self.zoom) * kz;
        View { x: self.x, y: self.y, zoom: self.zoom, shake: fx.shake_offset() }
    }
}
