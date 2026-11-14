//! Controls for games: keyboard, mouse buttons and game controllers
//! (gamepads, joysticks) through one list of named actions.
//!
//! A game never asks "is Space down?" or "is the A button down?". It names its
//! actions once, in plain text, and asks about those:
//!
//! ```text
//! # NAME = where it comes from, separated by commas
//! throttle = key W, key ArrowUp, pad RT
//! lean     = keys D A, keys ArrowRight ArrowLeft, stick LeftX, pads Right Left
//! fire     = mouse Primary, pad RB
//! start    = key Space, pad Start
//! ```
//!
//! ```ignore
//! let mut input = Input::load_or("assets/input/my-game.input", DEFAULT)?;
//! // each frame:
//! input.update(ctx);
//! if input.pressed("start") { ... }      // went down this frame
//! let gas = input.value("throttle");      // 0..1 (analogue triggers too)
//! let lean = input.axis("lean");          // -1..1
//! ```
//!
//! Sources:
//! - `key NAME`: a keyboard key (egui names: `A`, `Space`, `ArrowUp`, `Enter`, `Num1`...).
//! - `keys POS NEG`: two keys as one axis (+1 / -1).
//! - `mouse Primary|Secondary|Middle`: a mouse button.
//! - `pad BUTTON`: a controller button, 0..1 (triggers are analogue).
//!   Names: `A B X Y` (Xbox layout; also `South East West North`), `LB RB LT RT`,
//!   `Back Start Home`, `LStick RStick` (pressing the sticks), `Up Down Left Right` (d-pad).
//! - `pads POS NEG`: two controller buttons as one axis.
//! - `stick AXIS`: a stick, -1..1: `LeftX LeftY RightX RightY` (Y is **down-positive**,
//!   like the screen), `LeftZ RightZ` (some controllers' triggers). Add `-` to flip: `stick LeftY-`.
//!
//! Every source feeding an action is read; the strongest wins, so keyboard and
//! controller work side by side. Sticks have a dead zone (small wobbles are 0).
//! The first connected controller is used; plugging one in mid-game just works.
//! Players can remap by editing the game's `.input` file; no code changes.

use eframe::egui;
use gilrs::{Axis, Button};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Where an action's value comes from.
#[derive(Clone, Debug, PartialEq)]
enum Source {
    Key(egui::Key),
    Keys(egui::Key, egui::Key),
    Mouse(egui::PointerButton),
    Pad(Button),
    Pads(Button, Button),
    Stick(Axis, f32),
}

/// What a controller is doing right now (buttons 0..1, sticks -1..1).
#[derive(Clone, Debug, Default)]
pub struct PadState {
    pub buttons: HashMap<Button, f32>,
    pub axes: HashMap<Axis, f32>,
}

/// What the keyboard and mouse are doing right now.
#[derive(Clone, Debug, Default)]
pub struct KeyState {
    pub keys: Vec<egui::Key>,
    pub mouse: Vec<egui::PointerButton>,
}

/// A game's controls: named actions, read from keyboard, mouse and controller.
pub struct Input {
    actions: BTreeMap<String, Vec<Source>>,
    now: BTreeMap<String, f32>,
    before: BTreeMap<String, f32>,
    /// Stick values below this count as zero.
    pub dead_zone: f32,
    gilrs: Option<gilrs::Gilrs>,
    pad: Option<gilrs::GamepadId>,
    pad_name: Option<String>,
}

fn key(name: &str) -> Result<egui::Key, String> {
    // egui's own names, plus friendlier ones: Num1 (the 1 key), Up, Esc...
    let alias = match name.to_ascii_lowercase().as_str() {
        "up" => "ArrowUp",
        "down" => "ArrowDown",
        "left" => "ArrowLeft",
        "right" => "ArrowRight",
        "esc" => "Escape",
        "return" => "Enter",
        _ => name.strip_prefix("Num").filter(|d| d.len() == 1).unwrap_or(name),
    };
    egui::Key::from_name(alias).ok_or_else(|| format!("unknown key `{name}` (use names like A, Space, ArrowUp, Enter, Escape, Num1)"))
}

fn button(name: &str) -> Result<Button, String> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "a" | "south" | "cross" => Button::South,
        "b" | "east" | "circle" => Button::East,
        "x" | "west" | "square" => Button::West,
        "y" | "north" | "triangle" => Button::North,
        "lb" | "l1" => Button::LeftTrigger,
        "rb" | "r1" => Button::RightTrigger,
        "lt" | "l2" => Button::LeftTrigger2,
        "rt" | "r2" => Button::RightTrigger2,
        "back" | "select" | "view" | "share" => Button::Select,
        "start" | "menu" | "options" => Button::Start,
        "home" | "guide" | "mode" => Button::Mode,
        "lstick" | "l3" => Button::LeftThumb,
        "rstick" | "r3" => Button::RightThumb,
        "up" => Button::DPadUp,
        "down" => Button::DPadDown,
        "left" => Button::DPadLeft,
        "right" => Button::DPadRight,
        _ => return Err(format!("unknown controller button `{name}` (use A B X Y, LB RB LT RT, Back Start Home, LStick RStick, Up Down Left Right)")),
    })
}

fn stick(name: &str) -> Result<(Axis, f32), String> {
    let (name, flip) = match name.strip_suffix('-') {
        Some(n) => (n, -1.0),
        None => (name, 1.0),
    };
    // gilrs counts stick Y as up-positive; games here use screen coordinates
    // (down-positive), so Y is flipped once here.
    Ok(match name.to_ascii_lowercase().as_str() {
        "leftx" => (Axis::LeftStickX, flip),
        "lefty" => (Axis::LeftStickY, -flip),
        "rightx" => (Axis::RightStickX, flip),
        "righty" => (Axis::RightStickY, -flip),
        "leftz" => (Axis::LeftZ, flip),
        "rightz" => (Axis::RightZ, flip),
        _ => return Err(format!("unknown stick `{name}` (use LeftX LeftY RightX RightY LeftZ RightZ, `-` on the end to flip)")),
    })
}

fn mouse(name: &str) -> Result<egui::PointerButton, String> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "primary" | "left" => egui::PointerButton::Primary,
        "secondary" | "right" => egui::PointerButton::Secondary,
        "middle" => egui::PointerButton::Middle,
        _ => return Err(format!("unknown mouse button `{name}` (use Primary, Secondary or Middle)")),
    })
}

impl Input {
    /// Read a list of actions (see the top of this file). No controller is
    /// opened: call `connect_controllers` (done by `load_or`) for real play.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut actions = BTreeMap::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let err = |e: String| format!("line {}: {e}", n + 1);
            let (name, rest) = line.split_once('=').ok_or_else(|| err("use `NAME = source, source`".into()))?;
            let name = name.trim();
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err(err(format!("action names use letters, numbers and _: `{name}`")));
            }
            let mut sources = Vec::new();
            for part in rest.split(',') {
                let w: Vec<_> = part.split_whitespace().collect();
                let s = match w.as_slice() {
                    ["key", k] => Source::Key(key(k).map_err(err)?),
                    ["keys", p, m] => Source::Keys(key(p).map_err(err)?, key(m).map_err(err)?),
                    ["mouse", b] => Source::Mouse(mouse(b).map_err(err)?),
                    ["pad", b] => Source::Pad(button(b).map_err(err)?),
                    ["pads", p, m] => Source::Pads(button(p).map_err(err)?, button(m).map_err(err)?),
                    ["stick", a] => {
                        let (axis, sign) = stick(a).map_err(err)?;
                        Source::Stick(axis, sign)
                    }
                    [] => continue,
                    _ => return Err(err(format!("don't know `{}` (use key, keys, mouse, pad, pads or stick)", part.trim()))),
                };
                sources.push(s);
            }
            if actions.insert(name.to_string(), sources).is_some() {
                return Err(err(format!("`{name}` is listed twice")));
            }
        }
        Ok(Input { actions, now: BTreeMap::new(), before: BTreeMap::new(), dead_zone: 0.18, gilrs: None, pad: None, pad_name: None })
    }

    /// The player's own file if there is one, otherwise the game's defaults;
    /// controllers connected. A mistake in the file is reported, not ignored.
    pub fn load_or(path: impl AsRef<Path>, default: &str) -> Result<Self, String> {
        let path = path.as_ref();
        let mut input = match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?,
            Err(_) => Self::parse(default)?,
        };
        input.connect_controllers();
        Ok(input)
    }

    /// Start listening for game controllers (harmless if there are none).
    pub fn connect_controllers(&mut self) {
        if self.gilrs.is_none() {
            self.gilrs = gilrs::Gilrs::new().ok();
        }
    }

    /// The controller in use, if one is plugged in (for "Controller: ..." hints).
    pub fn controller(&self) -> Option<&str> {
        self.pad_name.as_deref()
    }

    /// The actions this game has.
    pub fn actions(&self) -> impl Iterator<Item = &str> {
        self.actions.keys().map(String::as_str)
    }

    /// Read this frame's keyboard, mouse and controller. Call once a frame.
    pub fn update(&mut self, ctx: &egui::Context) {
        let keys = ctx.input(|i| KeyState {
            keys: i.keys_down.iter().copied().collect(),
            mouse: [egui::PointerButton::Primary, egui::PointerButton::Secondary, egui::PointerButton::Middle]
                .into_iter()
                .filter(|b| i.pointer.button_down(*b))
                .collect(),
        });
        let pad = self.poll_controller();
        self.update_from(&keys, &pad);
    }

    fn poll_controller(&mut self) -> PadState {
        let Some(g) = &mut self.gilrs else { return PadState::default() };
        while g.next_event().is_some() {}
        if self.pad.is_none_or(|id| !g.gamepad(id).is_connected()) {
            self.pad = g.gamepads().find(|(_, p)| p.is_connected()).map(|(id, _)| id);
            self.pad_name = self.pad.map(|id| g.gamepad(id).name().to_string());
        }
        let Some(id) = self.pad else { return PadState::default() };
        let p = g.gamepad(id);
        let mut state = PadState::default();
        for b in [
            Button::South, Button::East, Button::North, Button::West, Button::LeftTrigger, Button::RightTrigger,
            Button::LeftTrigger2, Button::RightTrigger2, Button::Select, Button::Start, Button::Mode,
            Button::LeftThumb, Button::RightThumb, Button::DPadUp, Button::DPadDown, Button::DPadLeft, Button::DPadRight,
        ] {
            let v = p.button_data(b).map_or(if p.is_pressed(b) { 1.0 } else { 0.0 }, |d| d.value());
            state.buttons.insert(b, v);
        }
        for a in [Axis::LeftStickX, Axis::LeftStickY, Axis::RightStickX, Axis::RightStickY, Axis::LeftZ, Axis::RightZ] {
            state.axes.insert(a, p.value(a));
        }
        state
    }

    /// Work out every action from given keyboard/mouse and controller states
    /// (`update` does this with the real ones; tests and replays can feed their own).
    pub fn update_from(&mut self, keys: &KeyState, pad: &PadState) {
        self.before = std::mem::take(&mut self.now);
        let dz = self.dead_zone.clamp(0.0, 0.95);
        let held = |k: &egui::Key| keys.keys.contains(k) as u8 as f32;
        let btn = |b: &Button| pad.buttons.get(b).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        for (name, sources) in &self.actions {
            let mut best = 0.0f32;
            for s in sources {
                let v = match s {
                    Source::Key(k) => held(k),
                    Source::Keys(p, m) => held(p) - held(m),
                    Source::Mouse(b) => keys.mouse.contains(b) as u8 as f32,
                    Source::Pad(b) => btn(b),
                    Source::Pads(p, m) => btn(p) - btn(m),
                    Source::Stick(a, sign) => {
                        let raw = pad.axes.get(a).copied().unwrap_or(0.0).clamp(-1.0, 1.0);
                        // Dead zone, then rescale so the edge of it is 0, not a jump.
                        let v = if raw.abs() <= dz { 0.0 } else { raw.signum() * (raw.abs() - dz) / (1.0 - dz) };
                        v * sign
                    }
                };
                if v.abs() > best.abs() {
                    best = v;
                }
            }
            self.now.insert(name.clone(), best);
        }
    }

    /// An action's value: -1..1 for axes, 0..1 for buttons (unknown names: 0).
    pub fn axis(&self, name: &str) -> f32 {
        self.now.get(name).copied().unwrap_or(0.0)
    }

    /// The same as `axis`, for buttons and triggers (0..1); negative values read as 0.
    pub fn value(&self, name: &str) -> f32 {
        self.axis(name).max(0.0)
    }

    /// Is it held (more than half way)?
    pub fn down(&self, name: &str) -> bool {
        self.axis(name).abs() > 0.5
    }

    /// Did it go down this frame?
    pub fn pressed(&self, name: &str) -> bool {
        self.down(name) && self.before.get(name).is_none_or(|v| v.abs() <= 0.5)
    }

    /// Did it come up this frame?
    pub fn released(&self, name: &str) -> bool {
        !self.down(name) && self.before.get(name).is_some_and(|v| v.abs() > 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAME: &str = "throttle = key W, pad RT\nlean = keys D A, stick LeftX, pads Right Left\nclimb = stick LeftY-\nfire = mouse Primary, pad A\n";

    #[test]
    fn keyboard_and_controller_feed_the_same_actions() {
        let mut i = Input::parse(GAME).unwrap();
        let none = PadState::default();
        i.update_from(&KeyState { keys: vec![egui::Key::W, egui::Key::A], mouse: vec![] }, &none);
        assert_eq!((i.value("throttle"), i.axis("lean")), (1.0, -1.0));
        assert!(i.pressed("throttle") && !i.pressed("fire"));
        // Now the controller: half trigger, stick pushed right, A pressed.
        let mut pad = PadState::default();
        pad.buttons.insert(Button::RightTrigger2, 0.5);
        pad.axes.insert(Axis::LeftStickX, 0.8);
        pad.axes.insert(Axis::LeftStickY, 1.0); // pushed up (gilrs: up is +)
        pad.buttons.insert(Button::South, 1.0);
        i.update_from(&KeyState::default(), &pad);
        assert_eq!(i.value("throttle"), 0.5, "analogue trigger");
        assert!(i.axis("lean") > 0.7 && i.axis("lean") < 0.8, "stick, dead zone rescaled: {}", i.axis("lean"));
        assert_eq!(i.axis("climb"), 1.0, "LeftY- means up is +1");
        assert!(i.pressed("fire") && i.released("throttle"));
        i.update_from(&KeyState::default(), &pad);
        assert!(!i.pressed("fire") && i.down("fire"), "pressed only once");
    }

    #[test]
    fn small_stick_wobble_is_ignored_and_strongest_source_wins() {
        let mut i = Input::parse(GAME).unwrap();
        let mut pad = PadState::default();
        pad.axes.insert(Axis::LeftStickX, 0.1);
        i.update_from(&KeyState::default(), &pad);
        assert_eq!(i.axis("lean"), 0.0);
        i.update_from(&KeyState { keys: vec![egui::Key::D], mouse: vec![] }, &pad);
        assert_eq!(i.axis("lean"), 1.0);
        assert_eq!(i.axis("no_such_action"), 0.0);
    }

    #[test]
    fn the_games_control_files_are_valid() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/input");
        let mut n = 0;
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "input") {
                let text = std::fs::read_to_string(&p).unwrap();
                Input::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                n += 1;
            }
        }
        assert!(n >= 2);
        let mut i = Input::parse("one = key Num1
back = key Esc
").unwrap();
        i.update_from(&KeyState { keys: vec![egui::Key::Num1, egui::Key::Escape], mouse: vec![] }, &PadState::default());
        assert!(i.down("one") && i.down("back"));
    }

    #[test]
    fn mistakes_name_the_line() {
        for (text, needle) in [
            ("jump = key Spcae", "line 1: unknown key"),
            ("ok = key A\njump = pad Z9", "line 2: unknown controller button"),
            ("jump key Space", "line 1"),
            ("jump = key A\njump = key B", "twice"),
            ("look = stick Middle", "unknown stick"),
        ] {
            let e = Input::parse(text).err().unwrap();
            assert!(e.contains(needle), "{text}: {e}");
        }
    }
}
