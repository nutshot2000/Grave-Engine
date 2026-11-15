//! Soaked characters: walking through water or oil (or getting rained on,
//! or splashed) soaks a character's clothes. It dries off over time.
//!
//! - Soaked in something that puts fire out (water): heat and fire hurt less,
//!   and a hot body steams it off.
//! - Soaked in something flammable (oil): heat or flames set the character
//!   alight. It burns until the oil is gone or it jumps into water.
//! - A good soaking drips: an oily character leaves a trail (that can burn).
//!
//! A deep dunk (half the body under) washes off whatever was there before:
//! jump in the lake to get the oil off. Shown on the character (its cells get
//! the liquid's film look, pictures are tinted) and with an icon and HUD line.

use super::*;

/// Characters dry this many times faster than the ground does (warm, moving).
const BODY_DRIES: f32 = 3.0;
/// How long a full oil soaking burns for (seconds).
const BURNS_FOR: f32 = 6.0;
/// Damage per second while on fire.
const FIRE_HURTS: f32 = 14.0;

/// What a character is soaked in, and how much (0..1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Soak {
    pub liquid: MatId,
    pub amount: f32,
    /// On fire (a flammable soaking that caught).
    pub burning: bool,
}

/// A number 0..1 from a seed (repeatable, so replays match).
fn dice(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xffff) as f32 / 65535.0
}

/// How much of the heat damage still gets through (a wet body is protected).
pub(super) fn heat_shield(spec: &ActorSpec, mats: &Materials) -> f32 {
    match spec.soak {
        Some(s) if mats.get(s.liquid).puts_out_fire => 1.0 - 0.6 * s.amount,
        _ => 1.0,
    }
}

/// One frame of soaking, drying and burning. Returns damage to add.
pub(super) fn update(a: &mut Actor, world: &mut World, mats: &Materials, cx: f32, cy: f32, n: usize) -> f32 {
    let frame = world.frame;
    let seed = frame.wrapping_mul(31).wrapping_add(n as u32 * 7919);
    let left = (cx - W as f32 / 2.0).round() as i32;
    let top = (cy - H as f32 / 2.0).round() as i32;
    let (ww, wh) = (world.w as i32, world.h as i32);
    let cell = move |x: i32, y: i32| (x >= 0 && y >= 0 && x < ww && y < wh).then(|| y as usize * ww as usize + x as usize);

    // What liquid is beside (and under) the body, and is there fire?
    let mut found: Vec<(MatId, u32)> = Vec::new();
    let mut flames = false;
    let mut note = |i: usize, found: &mut Vec<(MatId, u32)>| {
        if world.is_owned(i) {
            return;
        }
        let m = mats.get(world.mat[i]);
        if m.state == State::Liquid {
            match found.iter_mut().find(|e| e.0 == world.mat[i]) {
                Some(e) => e.1 += 1,
                None => found.push((world.mat[i], 1)),
            }
        }
        flames |= world.is_burning(i) || (m.state == State::Gas && m.glow > 0.3 && m.feeds_fire);
    };
    for row in -1..=H as i32 {
        for x in [left - 1, left + W as i32] {
            if let Some(i) = cell(x, top + row) {
                note(i, &mut found);
            }
        }
    }
    for x in left..left + W as i32 {
        for y in [top - 1, top + H as i32] {
            if let Some(i) = cell(x, y) {
                note(i, &mut found);
            }
        }
    }
    let touching = found.iter().max_by_key(|e| e.1).map(|e| e.0);

    // Rain and splashes: liquid flying into the body.
    let mut spray: Option<MatId> = None;
    let mut drops = 0u32;
    for f in &world.flying {
        if (f.x - cx).abs() < W as f32 / 2.0 + 1.5 && (f.y - cy).abs() < H as f32 / 2.0 + 1.5 && mats.get(f.mat).leaves_film() {
            spray = Some(f.mat);
            drops += 1;
        }
    }

    let soak = &mut a.spec.soak;
    // A liquid that puts fire out douses a burning character.
    if let Some(s) = soak.as_mut()
        && s.burning
        && touching.or(spray).is_some_and(|l| mats.get(l).puts_out_fire)
        && (a.wet > 0.2 || drops > 2)
    {
        s.burning = false;
    }
    // Getting soaked (a dunk washes off what was there; a splash only tops
    // up the same liquid, or a dry character).
    let mut soak_in = |liquid: MatId, gain: f32, dunk: bool| {
        if !mats.get(liquid).leaves_film() {
            return;
        }
        match soak {
            Some(s) if s.liquid == liquid => s.amount = (s.amount + gain).min(1.0),
            Some(s) if dunk || s.amount < 0.05 => *s = Soak { liquid, amount: gain.min(1.0), burning: false },
            Some(_) => {}
            None => *soak = Some(Soak { liquid, amount: gain.min(1.0), burning: false }),
        }
    };
    if let Some(l) = touching {
        soak_in(l, DT * (0.4 + 3.0 * a.wet), a.wet >= 0.5);
    }
    if let Some(l) = spray {
        soak_in(l, 0.004 * drops as f32, false);
    }

    let Some(s) = soak.as_mut() else { return 0.0 };
    let liquid = mats.get(s.liquid);
    let hottest = a.stamped.iter().map(|&(_, li)| a.temps[li as usize]).fold(a.spec.temp, f32::max);
    let mut hurt = 0.0;

    // Drying: over the liquid's own time, faster on a hot body; a hot body
    // boils water off in a puff of steam.
    s.amount -= DT * BODY_DRIES / liquid.coat_lasts.max(0.5);
    if let Some(boil) = &liquid.boils
        && hottest >= boil.at * 0.6
    {
        s.amount -= DT * 0.8;
        if dice(seed) < 0.3 {
            world.spawn_flyer(mats, boil.into, cx + (dice(seed ^ 3) - 0.5) * W as f32, top as f32 + 2.0, 0.0, -0.3);
        }
    }

    // Flammable: heat or flames set it alight; then it burns the oil away.
    if let Some(ignites) = liquid.catches_fire_at {
        if !s.burning && s.amount > 0.05 && (hottest >= ignites.min(120.0) || flames) {
            s.burning = true;
        }
        if s.burning {
            s.amount -= DT / BURNS_FOR;
            hurt += FIRE_HURTS * DT;
            // Flames lick up off the body (flying, so they rise away instead
            // of sitting against it: the burning hurts at a steady rate).
            if let Some(flame) = liquid.gives_off {
                for k in 0..2u32 {
                    if dice(seed ^ (7 + k * 101)) < 0.7 {
                        let fx = cx + (dice(seed ^ (11 + k * 101)) - 0.5) * (W as f32 + 1.0);
                        let fy = top as f32 + dice(seed ^ (13 + k * 101)) * H as f32 * 0.8;
                        let sway = (dice(seed ^ (23 + k * 101)) - 0.5) * 0.3;
                        if cell(fx as i32, fy as i32).is_some_and(|i| world.mat[i] == crate::material::AIR) {
                            world.spawn_flyer(mats, flame, fx, fy, sway + a.spec.vx * 0.01, -0.5 - dice(seed ^ (29 + k * 101)) * 0.5);
                        }
                    }
                }
            }
        }
    }

    // A good soaking drips.
    if s.amount > 0.3 && !s.burning && a.wet < 0.1 && dice(seed ^ 17) < s.amount * 0.05 {
        s.amount -= 0.01;
        let fx = cx + (dice(seed ^ 19) - 0.5) * (W as f32 - 1.0);
        world.spawn_flyer(mats, s.liquid, fx, (top + H as i32) as f32 + 0.5, 0.0, 0.2);
    }

    if s.amount <= 0.0 {
        *soak = None;
    }
    hurt
}

/// Give the figure's cells the look of its soaking (see world/coating.rs).
pub(super) fn mark_cells(spec: &ActorSpec, world: &mut World, mats: &Materials, cells: &[(u32, u16)]) {
    let Some(s) = spec.soak else { return };
    let frames = (mats.get(s.liquid).coat_lasts * 60.0 * s.amount.clamp(0.0, 1.0)) as u32;
    for &(wi, _) in cells {
        world.coat[wi as usize] = s.liquid;
        world.coat_until[wi as usize] = world.frame.saturating_add(frames.max(1));
    }
}
