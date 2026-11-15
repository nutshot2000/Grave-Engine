//! Sound from materials. The simulation notes down what happened each frame
//! (a hit, a break, a blast, thunder, something landing) as `Sound` cues on
//! the world; the player (sound/play.rs) turns them into noise made up on
//! the spot from what the things are made of. No sound files.
//!
//! Every material has a voice: metal rings, wood knocks, stone thuds, glass
//! chimes, soft things (rubber, flesh, cloth) thump, powders crunch, liquids
//! splash, gases are silent. A material file can choose its own with
//! `sound = metal|wood|stone|glass|soft|earth|liquid|none`; without one it is
//! worked out from the material (metal = yes, state, name, burns...).
//!
//! Fire crackles, rain, hail and wind are heard from the state of the world
//! (what's burning on screen, the weather), not from cues.
//!
//! Games and the editor: keep a `Player` and call `player.listen(&mut sim, view, speed)`
//! once a frame after stepping. It drains the cues, so they never pile up.

pub mod play;

use crate::material::{MatId, Material, Materials, State};

pub use play::Player;

/// What kind of thing happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Two things hit each other (`mat` and `other`).
    Hit,
    /// An object broke into pieces (or burned through).
    Break,
    /// An explosion, `size` cells across.
    Blast,
    /// A lightning strike.
    Thunder,
    /// A flying cell (spray, debris, a hailstone) landed.
    Land,
}

/// One thing heard this frame.
#[derive(Clone, Copy, Debug)]
pub struct Sound {
    pub kind: Kind,
    /// Where, in cells.
    pub x: f32,
    pub y: f32,
    /// What made it, and what it hit (for a `Hit`).
    pub mat: MatId,
    pub other: MatId,
    /// How hard, 0 (barely) to 1 (as loud as it gets).
    pub loud: f32,
    /// How big the thing is, in cells (bigger rings lower).
    pub size: f32,
}

/// Cues kept per world before the oldest stop counting (nobody listening).
pub const MAX_CUES: usize = 160;

/// How a material sounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Voice {
    Metal,
    Wood,
    Stone,
    Glass,
    Soft,
    Earth,
    Liquid,
    Silent,
}

impl Voice {
    pub const ALL: [Voice; 8] = [Voice::Metal, Voice::Wood, Voice::Stone, Voice::Glass, Voice::Soft, Voice::Earth, Voice::Liquid, Voice::Silent];

    pub fn name(self) -> &'static str {
        match self {
            Voice::Metal => "metal",
            Voice::Wood => "wood",
            Voice::Stone => "stone",
            Voice::Glass => "glass",
            Voice::Soft => "soft",
            Voice::Earth => "earth",
            Voice::Liquid => "liquid",
            Voice::Silent => "none",
        }
    }

    pub fn parse(s: &str) -> Option<Voice> {
        let s = s.trim().to_lowercase();
        Voice::ALL.into_iter().find(|v| v.name() == s).or(match s.as_str() {
            "silent" | "off" => Some(Voice::Silent),
            "rock" | "brick" => Some(Voice::Stone),
            "water" => Some(Voice::Liquid),
            "powder" | "sand" | "dirt" => Some(Voice::Earth),
            _ => None,
        })
    }
}

/// How this material sounds: its own `sound =` setting, or worked out.
pub fn voice(m: &Material) -> Voice {
    if !m.sound.is_empty()
        && let Some(v) = Voice::parse(&m.sound)
    {
        return v;
    }
    let name = m.name.as_str();
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    match m.state {
        State::Gas => Voice::Silent,
        State::Liquid => Voice::Liquid,
        State::Powder if m.lifetime.is_some() || m.glow > 0.3 => Voice::Silent,
        State::Powder => Voice::Earth,
        State::Solid => {
            if m.metal {
                Voice::Metal
            } else if has(&["glass", "ice", "crystal", "ceramic", "porcelain"]) {
                Voice::Glass
            } else if has(&["rubber", "flesh", "cloth", "fabric", "leather", "foam", "wool", "straw", "bounce", "plastic"]) {
                Voice::Soft
            } else if has(&["mud", "dirt", "snow"]) {
                Voice::Earth
            } else if m.catches_fire_at.is_some() {
                Voice::Wood
            } else {
                Voice::Stone
            }
        }
    }
}

/// Every material's voice, looked up once.
pub fn voices(mats: &Materials) -> Vec<Voice> {
    mats.list.iter().map(voice).collect()
}

impl crate::world::World {
    /// Note down something heard this frame (see sound.rs). Quiet ones are
    /// dropped once there are plenty.
    pub fn sound(&mut self, s: Sound) {
        if !(s.loud > 0.0) || !s.x.is_finite() || !s.y.is_finite() {
            return;
        }
        if self.sounds.len() >= MAX_CUES {
            // Full: a louder one replaces the quietest.
            if let Some((n, q)) = self.sounds.iter().enumerate().min_by(|a, b| a.1.loud.total_cmp(&b.1.loud)) {
                if q.loud < s.loud {
                    self.sounds[n] = s;
                }
            }
            return;
        }
        self.sounds.push(Sound { loud: s.loud.min(1.0), ..s });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_material_has_a_sensible_voice() {
        let mats = Materials::load_dir("materials");
        let v = |n: &str| voice(mats.get(mats.id(n).unwrap()));
        assert_eq!(v("iron"), Voice::Metal);
        assert_eq!(v("brass"), Voice::Metal);
        assert_eq!(v("wood"), Voice::Wood);
        assert_eq!(v("stone"), Voice::Stone);
        assert_eq!(v("brick"), Voice::Stone);
        assert_eq!(v("glass"), Voice::Glass);
        assert_eq!(v("ice"), Voice::Glass);
        assert_eq!(v("rubber"), Voice::Soft);
        assert_eq!(v("flesh"), Voice::Soft);
        assert_eq!(v("sand"), Voice::Earth);
        assert_eq!(v("water"), Voice::Liquid);
        assert_eq!(v("smoke"), Voice::Silent);
        assert_eq!(v("fire"), Voice::Silent);
    }

    #[test]
    fn a_full_list_keeps_the_loud_ones() {
        let mut w = crate::world::World::new(10, 10);
        let cue = |loud| Sound { kind: Kind::Land, x: 1.0, y: 1.0, mat: 0, other: 0, loud, size: 1.0 };
        for _ in 0..MAX_CUES + 50 {
            w.sound(cue(0.1));
        }
        assert_eq!(w.sounds.len(), MAX_CUES);
        w.sound(cue(0.9));
        assert!(w.sounds.iter().any(|s| s.loud == 0.9));
        w.sound(cue(f32::NAN));
        assert_eq!(w.sounds.len(), MAX_CUES);
    }
}
