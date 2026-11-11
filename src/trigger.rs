//! Triggers: boxes in a scene that do something when something enters them.
//! Level events without code: light a fuse as the bike gets close, mark a
//! checkpoint, award points, show a message.
//!
//! ```text
//! trigger X Y W H [by NAME|player|any] [repeat] : ACTION ; ACTION ...
//! trigger 1535 150 10 60 by frame : heat circle 1508 205 8 1200 ; message DEMOLITION CHARGE!
//! trigger 1340 100 10 120 by frame : checkpoint ; score 500 ; message THE BOG — cleared!
//! ```
//!
//! - `by NAME`: a named object (the default `any` is any object or
//!   character; `player` is characters only). It fires when the thing's
//!   centre enters the box.
//! - It fires once (`repeat`: every time something enters).
//! - Actions: `heat circle X Y R TEMP` / `heat rect X Y W H TEMP` (set the
//!   temperature: real fire and explosions follow from the material rules),
//!   `message TEXT`, `checkpoint [X Y]` (the place to come back to: X Y,
//!   or the middle of the box),
//!   `score N`.
//! - Camera actions (see camera.rs), each for S seconds (left out or 0 =
//!   until something changes it): `slowmo F [S]` (time at F: 0.3 = 30 %),
//!   `zoom Z [S]` (Z times closer), `focus [S]` (look at the box),
//!   `focus X Y [S]`, `focus it [S]` (look at whatever set it off),
//!   `shake N` (N cells), `camera reset` (back to normal).
//! - Lighting: `lighting L [S]` fades the overall light to L (0 dark .. 1
//!   full) over S seconds (see light.rs).
//! - `sun ANGLE STRENGTH [#colour] [S]` moves the sun over S seconds (a sunset).
//! - `finish`: the end of the level (games read `Event::Finish`).
//!
//! Games read what happened from `Sim::events` (and `Sim::checkpoint`).

use crate::world::World;

/// Who sets a trigger off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum By {
    /// Any object or character.
    Any,
    /// Characters only.
    Player,
    /// One object: its id (in a running sim) or its number N+1 (in a
    /// snapshot or scene, like joints).
    Object(u64),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Set temperature in a circle (cx, cy, r) or rectangle (x, y, w, h).
    HeatCircle(i32, i32, i32, f32),
    HeatRect(i32, i32, i32, i32, f32),
    Message(String),
    /// Come back here (or at the trigger's centre).
    Checkpoint(Option<(f32, f32)>),
    Score(i64),
    /// Camera: time scale for seconds; zoom for seconds; look at something
    /// for seconds; shake; back to normal.
    SlowMo(f32, f32),
    Zoom(f32, f32),
    Focus(FocusAt, f32),
    Shake(f32),
    CameraReset,
    /// Fade the overall light to a level over seconds.
    Lighting(f32, f32),
    /// The end of the level (the game decides what that means).
    Finish,
    /// Move the sun (angle, strength, colour) over seconds.
    Sun(crate::light::Sun, f32),
}

/// What a `focus` action looks at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FocusAt {
    /// The middle of the trigger's box.
    Box,
    Point(f32, f32),
    /// Whatever set the trigger off.
    It,
}

/// Something a trigger did, for the game to show or count.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Message(String),
    /// A checkpoint at the trigger's centre (cells).
    Checkpoint(f32, f32),
    Score(i64),
    /// The level's finish line was reached.
    Finish,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Trigger {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub by: By,
    pub repeat: bool,
    pub actions: Vec<Action>,
    /// Has it gone off (once-only triggers then stay quiet).
    pub fired: bool,
    /// Something was inside last frame (so `repeat` fires on entering).
    pub occupied: bool,
}

impl Trigger {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    /// Do its actions: changes to the world now, events for the game.
    /// `who` = what set it off (for `focus it`).
    pub fn fire(&self, world: &mut World, events: &mut Vec<Event>, camera: &mut crate::camera::Effects, lighting: &mut crate::light::Lighting, who: Option<crate::camera::Focus>) {
        use crate::camera::Focus;
        for a in &self.actions {
            match a {
                Action::HeatCircle(cx, cy, r, t) => {
                    let (x0, x1, y0, y1) = world.clip(cx - r, cx + r, cy - r, cy + r);
                    for y in y0..=y1 {
                        for x in x0..=x1 {
                            if (x - cx).pow(2) + (y - cy).pow(2) <= r * r + r / 2 {
                                let i = world.idx(x as usize, y as usize);
                                world.temp[i] = *t;
                                world.wake_at(i);
                            }
                        }
                    }
                }
                Action::HeatRect(x, y, w, h, t) => {
                    let (x0, x1, y0, y1) = world.clip(*x, x + w - 1, *y, y + h - 1);
                    for yy in y0..=y1 {
                        for xx in x0..=x1 {
                            let i = world.idx(xx as usize, yy as usize);
                            world.temp[i] = *t;
                                world.wake_at(i);
                        }
                    }
                }
                Action::Message(m) => events.push(Event::Message(m.clone())),
                Action::Checkpoint(at) => {
                    let (x, y) = at.unwrap_or((self.x + self.w / 2.0, self.y + self.h / 2.0));
                    events.push(Event::Checkpoint(x, y))
                }
                Action::Score(n) => events.push(Event::Score(*n)),
                Action::SlowMo(f, s) => camera.slow_motion(*f, *s),
                Action::Zoom(z, s) => camera.zoom_in(*z, *s),
                Action::Focus(at, s) => {
                    let centre = Focus::Point(self.x + self.w / 2.0, self.y + self.h / 2.0);
                    let f = match at {
                        FocusAt::Box => centre,
                        FocusAt::Point(x, y) => Focus::Point(*x, *y),
                        FocusAt::It => who.unwrap_or(centre),
                    };
                    camera.look_at(f, *s);
                }
                Action::Shake(n) => camera.shake(*n),
                Action::CameraReset => camera.reset(),
                Action::Lighting(l, s) => lighting.fade_to(*l, *s),
                Action::Finish => events.push(Event::Finish),
                Action::Sun(to, s) => lighting.sun_to(*to, *s),
            }
        }
    }

    /// The scene line for this trigger (`by` already turned into a name).
    pub fn to_line(&self, by: &str) -> String {
        let mut s = format!("trigger {} {} {} {}", self.x, self.y, self.w, self.h);
        if by != "any" {
            s += &format!(" by {by}");
        }
        if self.repeat {
            s += " repeat";
        }
        if self.fired && !self.repeat {
            s += " fired";
        }
        let actions: Vec<String> = self
            .actions
            .iter()
            .map(|a| match a {
                Action::HeatCircle(x, y, r, t) => format!("heat circle {x} {y} {r} {t}"),
                Action::HeatRect(x, y, w, h, t) => format!("heat rect {x} {y} {w} {h} {t}"),
                Action::Message(m) => format!("message {m}"),
                Action::Checkpoint(None) => "checkpoint".into(),
                Action::Checkpoint(Some((x, y))) => format!("checkpoint {x} {y}"),
                Action::Score(n) => format!("score {n}"),
                Action::SlowMo(f, s) => format!("slowmo {f} {s}"),
                Action::Zoom(z, s) => format!("zoom {z} {s}"),
                Action::Focus(FocusAt::Box, s) => format!("focus {s}"),
                Action::Focus(FocusAt::Point(x, y), s) => format!("focus {x} {y} {s}"),
                Action::Focus(FocusAt::It, s) => format!("focus it {s}"),
                Action::Shake(n) => format!("shake {n}"),
                Action::CameraReset => "camera reset".into(),
                Action::Lighting(l, s) => format!("lighting {l} {s}"),
                Action::Finish => "finish".into(),
                Action::Sun(to, s) => format!("sun {} {} #{:02x}{:02x}{:02x} {s}", to.angle, to.strength, to.color[0], to.color[1], to.color[2]),
            })
            .collect();
        s + " : " + &actions.join(" ; ")
    }
}

/// Read a trigger line (without names resolved: `by` comes back as a word).
/// Returns the trigger (with `by` = Any for now) and the `by` word.
pub fn parse(words_before_colon: &[&str], actions: &str) -> Result<(Trigger, String), String> {
    let num = |i: usize| -> Result<f32, String> {
        words_before_colon.get(i).and_then(|w| w.parse::<f32>().ok()).ok_or("use `trigger X Y W H [by NAME] [repeat] : ACTIONS`".to_string())
    };
    let (x, y, w, h) = (num(1)?, num(2)?, num(3)?.max(1.0), num(4)?.max(1.0));
    let mut by = "any".to_string();
    let mut repeat = false;
    let mut fired = false;
    let mut i = 5;
    while i < words_before_colon.len() {
        match words_before_colon[i].to_lowercase().as_str() {
            "by" => {
                by = words_before_colon.get(i + 1).ok_or("`by` needs a name (or player / any)")?.to_lowercase();
                i += 1;
            }
            "repeat" => repeat = true,
            "once" => repeat = false,
            "fired" => fired = true,
            other => return Err(format!("trigger: don't know `{other}`")),
        }
        i += 1;
    }
    let mut list = Vec::new();
    for part in actions.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let w: Vec<&str> = part.split_whitespace().collect();
        let n = |k: usize| -> Result<f32, String> { w.get(k).and_then(|v| v.parse::<f32>().ok()).ok_or(format!("trigger action `{part}`: missing a number")) };
        list.push(match w[0].to_lowercase().as_str() {
            "heat" => match w.get(1).map(|s| s.to_lowercase()).as_deref() {
                Some("circle") => Action::HeatCircle(n(2)? as i32, n(3)? as i32, (n(4)? as i32).clamp(0, 4096), n(5)?.clamp(-273.0, 5000.0)),
                Some("rect") => Action::HeatRect(n(2)? as i32, n(3)? as i32, (n(4)? as i32).clamp(0, 8192), (n(5)? as i32).clamp(0, 8192), n(6)?.clamp(-273.0, 5000.0)),
                _ => return Err("use `heat circle X Y R TEMP` or `heat rect X Y W H TEMP`".into()),
            },
            "message" | "say" => Action::Message(part.split_once(char::is_whitespace).map(|p| p.1.trim().to_string()).unwrap_or_default()),
            "checkpoint" => Action::Checkpoint(if w.len() >= 3 { Some((n(1)?, n(2)?)) } else { None }),
            "score" | "points" => Action::Score(n(1)? as i64),
            "slowmo" | "slow" => Action::SlowMo(n(1)?.clamp(0.05, 1.0), n(2).unwrap_or(0.0).max(0.0)),
            "zoom" => Action::Zoom(n(1)?.clamp(0.1, 20.0), n(2).unwrap_or(0.0).max(0.0)),
            "focus" | "look" => {
                if w.get(1).is_some_and(|v| v.eq_ignore_ascii_case("it")) {
                    Action::Focus(FocusAt::It, n(2).unwrap_or(0.0).max(0.0))
                } else if w.len() >= 3 {
                    Action::Focus(FocusAt::Point(n(1)?, n(2)?), n(3).unwrap_or(0.0).max(0.0))
                } else {
                    Action::Focus(FocusAt::Box, n(1).unwrap_or(0.0).max(0.0))
                }
            }
            "shake" => Action::Shake(n(1)?.clamp(0.0, 20.0)),
            "finish" => Action::Finish,
            "sun" => {
                let color = w.get(3).filter(|c| c.starts_with('#')).map(|c| crate::material::parse_color(c)).transpose()?.unwrap_or([255, 240, 215]);
                let secs = w.get(if w.get(3).is_some_and(|c| c.starts_with('#')) { 4 } else { 3 }).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0).max(0.0);
                Action::Sun(crate::light::Sun { angle: n(1)?.clamp(-75.0, 75.0), strength: n(2)?.clamp(0.0, 5.0), color }, secs)
            }
            "lighting" | "lights" => Action::Lighting(n(1)?.clamp(0.0, 1.5), n(2).unwrap_or(0.0).max(0.0)),
            "camera" if w.get(1).is_some_and(|v| v.eq_ignore_ascii_case("reset")) => Action::CameraReset,
            other => return Err(format!("trigger: unknown action `{other}` (heat, message, checkpoint, score, slowmo, zoom, focus, shake, camera reset, lighting, sun, finish)")),
        });
    }
    if list.is_empty() {
        return Err("a trigger needs at least one action after `:`".into());
    }
    Ok((Trigger { x, y, w, h, by: By::Any, repeat, actions: list, fired, occupied: false }, by))
}
