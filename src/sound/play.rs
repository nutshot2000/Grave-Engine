//! Turning cues into sound (see sound.rs). Everything is made up on the
//! spot: ringing metal is a few out-of-tune sine waves fading at different
//! speeds, a wood knock is the same but quick and dull, stone and earth are
//! filtered noise, glass is high and bright, splashes are noise plus rising
//! bubble chirps, a blast is a deep sub-bass drop under a noise roar. Every
//! sound is rolled fresh, so no two hits are the same.
//!
//! Fire, rain, hail and wind are one endless stream whose levels follow
//! what's burning on screen and the weather.

use super::{Kind, Sound, Voice, voices};
use crate::material::Materials;
use crate::sim::Sim;
use rodio::{OutputStream, OutputStreamBuilder, Source, buffer::SamplesBuffer};
use std::f32::consts::TAU;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

pub const RATE: u32 = 22050;
/// Sounds playing at once, at most (quiet ones wait their turn: they're dropped).
const MAX_VOICES: usize = 24;
/// One-shot sounds started per frame, at most.
const PER_FRAME: usize = 6;

/// Plays a simulation's sounds. Keep one and call `listen` every frame.
pub struct Player {
    out: Option<OutputStream>,
    tried: bool,
    disabled: bool,
    voices: Vec<Voice>,
    ambience: Arc<Levels>,
    playing: Vec<Instant>,
    rng: Rng,
    /// 0..1, and a mute switch (the editor's 🔊 button).
    pub volume: f32,
    pub muted: bool,
}

/// The endless stream's levels, shared with the audio thread.
#[derive(Default)]
struct Levels {
    fire: AtomicU32,
    fire_pan: AtomicU32,
    rain: AtomicU32,
    hail: AtomicU32,
    wind: AtomicU32,
    volume: AtomicU32,
}

fn set(a: &AtomicU32, v: f32) {
    a.store(v.to_bits(), Ordering::Relaxed);
}
fn get(a: &AtomicU32) -> f32 {
    f32::from_bits(a.load(Ordering::Relaxed))
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

impl Player {
    /// A player that opens the speakers the first time there's something
    /// to hear (and stays silent if there are none, or `GRAVE_NO_SOUND` is set).
    pub fn new() -> Self {
        Player {
            out: None,
            tried: false,
            disabled: std::env::var_os("GRAVE_NO_SOUND").is_some(),
            voices: Vec::new(),
            ambience: Arc::new(Levels::default()),
            playing: Vec::new(),
            rng: Rng::new(0x2545_F491 ^ (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(1) | 1)),
            volume: 0.8,
            muted: false,
        }
    }

    /// A player that never makes a sound (tests, headless runs).
    pub fn silent() -> Self {
        Player { disabled: true, ..Self::new() }
    }

    fn open(&mut self) {
        if self.tried || self.disabled {
            return;
        }
        self.tried = true;
        let Ok(mut stream) = OutputStreamBuilder::open_default_stream() else { return };
        stream.log_on_drop(false);
        stream.mixer().add(Ambience::new(self.ambience.clone()));
        self.out = Some(stream);
    }

    /// Hear this frame: drain the cues the simulation noted (always, so
    /// they never pile up), play the ones near the view, and set the fire,
    /// rain and wind levels. `view` is the part of the world on screen
    /// (x0, y0, x1, y1) in cells; `speed` is how fast time runs (1 normal,
    /// below 1 slow motion: sounds drop in pitch; 0 paused: the world goes quiet).
    pub fn listen(&mut self, sim: &mut Sim, mats: &Materials, view: (f32, f32, f32, f32), speed: f32) {
        let mut cues = std::mem::take(&mut sim.world.sounds);
        for layer in &mut sim.layers {
            // Behind the front plane: muffled a little.
            cues.extend(std::mem::take(&mut layer.world.sounds).into_iter().map(|s| Sound { loud: s.loud * 0.55, ..s }));
        }
        let quiet = self.disabled || self.muted || self.volume <= 0.0;
        if quiet {
            set(&self.ambience.volume, 0.0);
            return;
        }
        if self.out.is_none() {
            if cues.is_empty() && !sim.weather.on && speed <= 0.0 {
                return;
            }
            self.open();
            if self.out.is_none() {
                return;
            }
        }
        if self.voices.len() != mats.list.len() {
            self.voices = voices(mats);
        }
        let running = speed > 0.0;
        self.ambient_levels(sim, view, running);
        if !running {
            return;
        }
        let pitch = speed.clamp(0.35, 1.0).sqrt();
        let now = Instant::now();
        self.playing.retain(|end| *end > now);
        for (cue, gain, pan) in self.choose(cues, view) {
            if self.playing.len() >= MAX_VOICES {
                break;
            }
            let v = |m: u16| self.voices.get(m as usize).copied().unwrap_or(Voice::Stone);
            let (voice, other) = (v(cue.mat), v(cue.other));
            if voice == Voice::Silent && !matches!(cue.kind, Kind::Blast | Kind::Thunder) {
                continue;
            }
            let mono = render(&cue, voice, other, &self.rng);
            if mono.is_empty() {
                continue;
            }
            let secs = mono.len() as f32 / RATE as f32 / pitch;
            let amp = gain * self.volume;
            let delay = if cue.kind == Kind::Thunder { rand(&self.rng) * 0.6 } else { 0.0 };
            let stereo = to_stereo(&mono, amp, pan);
            if let Some(out) = &self.out {
                out.mixer().add(SamplesBuffer::new(2, RATE, stereo).speed(pitch).delay(Duration::from_secs_f32(delay)));
            }
            self.playing.push(now + Duration::from_secs_f32(secs + delay));
        }
    }

    /// Which cues to play this frame, with their loudness and left-right
    /// position. Landings are gathered into one patter per kind of thing.
    fn choose(&mut self, cues: Vec<Sound>, view: (f32, f32, f32, f32)) -> Vec<(Sound, f32, f32)> {
        let (cx, cy) = ((view.0 + view.2) * 0.5, (view.1 + view.3) * 0.5);
        let (hw, hh) = (((view.2 - view.0) * 0.5).max(8.0), ((view.3 - view.1) * 0.5).max(8.0));
        let place = |s: &Sound| {
            let d = ((s.x - cx) / hw).abs().max(((s.y - cy) / hh).abs());
            // Big bangs carry: they fade slowly off screen.
            let carry = if matches!(s.kind, Kind::Blast | Kind::Thunder) { 0.35 } else { 1.6 };
            let fall = if d <= 1.0 { 1.0 } else { 1.0 / (1.0 + (d - 1.0) * carry).powi(2) };
            (fall, ((s.x - cx) / hw).clamp(-1.0, 1.0) * 0.75)
        };
        let mut out: Vec<(Sound, f32, f32)> = Vec::new();
        let mut patter: Vec<(Voice, Sound, u32)> = Vec::new();
        for s in cues {
            let (fall, pan) = place(&s);
            if fall * s.loud < 0.02 {
                continue;
            }
            if s.kind == Kind::Land {
                let v = self.voices.get(s.mat as usize).copied().unwrap_or(Voice::Stone);
                match patter.iter_mut().find(|p| p.0 == v) {
                    Some(p) => {
                        p.2 += 1;
                        if s.loud * fall > p.1.loud {
                            p.1 = Sound { loud: s.loud * fall, ..s };
                        }
                    }
                    None => patter.push((v, Sound { loud: s.loud * fall, ..s }, 1)),
                }
                continue;
            }
            out.push((s, s.loud * fall, pan));
        }
        for (_, s, n) in patter {
            let (_, pan) = place(&s);
            // Many at once is louder, but not n times louder.
            let loud = (s.loud * (1.0 + (n as f32).ln() * 0.35)).min(1.0);
            out.push((Sound { size: n as f32, ..s }, loud, pan));
        }
        // Big things first: a blast never waits behind a pebble.
        let rank = |k: Kind| match k {
            Kind::Thunder => 3.0,
            Kind::Blast => 2.0,
            Kind::Break => 1.2,
            _ => 1.0,
        };
        out.sort_by(|a, b| (b.1 * rank(b.0.kind)).total_cmp(&(a.1 * rank(a.0.kind))));
        // Several blasts in one frame sound as one big one.
        let mut blasts = 0;
        out.retain(|(s, _, _)| {
            if s.kind == Kind::Blast {
                blasts += 1;
                blasts <= 2
            } else {
                true
            }
        });
        out.truncate(PER_FRAME);
        out
    }

    /// Fire on screen, the weather: set the endless stream's levels.
    fn ambient_levels(&self, sim: &Sim, view: (f32, f32, f32, f32), running: bool) {
        let a = &self.ambience;
        set(&a.volume, self.volume);
        if !running {
            for l in [&a.fire, &a.rain, &a.hail, &a.wind] {
                set(l, 0.0);
            }
            return;
        }
        // Count burning cells, looking at a spread-out sample of the view.
        let w = &sim.world;
        let x0 = view.0.max(0.0) as usize;
        let y0 = view.1.max(0.0) as usize;
        let x1 = (view.2.max(0.0) as usize).min(w.w);
        let y1 = (view.3.max(0.0) as usize).min(w.h);
        let mut burning = 0.0f32;
        let mut sum_x = 0.0f32;
        if x1 > x0 && y1 > y0 {
            let area = ((x1 - x0) * (y1 - y0)) as f32;
            let step = ((area / 24000.0).sqrt().ceil() as usize).max(1);
            for y in (y0..y1).step_by(step) {
                for x in (x0..x1).step_by(step) {
                    let i = w.idx(x, y);
                    if w.burn[i] > 0 {
                        burning += 1.0;
                        sum_x += x as f32;
                    }
                }
            }
            if burning > 0.0 {
                let pan = ((sum_x / burning - (view.0 + view.2) * 0.5) / ((view.2 - view.0) * 0.5).max(1.0)).clamp(-1.0, 1.0) * 0.7;
                set(&a.fire_pan, pan);
                burning *= (step * step) as f32;
            }
        }
        set(&a.fire, ((1.0 + burning).log10() / 3.3).min(1.0));
        let wx = &sim.weather;
        let on = |v: f32| if wx.on { v.clamp(0.0, 1.0) } else { 0.0 };
        set(&a.rain, on(wx.rain));
        set(&a.hail, on(wx.hail));
        let wind = (w.wind.abs() / 18.0).min(1.0) * 0.8 + on(wx.gusts) * 0.4 + on(wx.dust) * 0.5;
        set(&a.wind, wind.min(1.0));
    }
}

/// The dice for making sounds (shared, so a sound can roll while building another).
pub type Rng = std::cell::Cell<u32>;

fn rand(r: &Rng) -> f32 {
    let mut x = r.get();
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    r.set(x);
    (x >> 8) as f32 / 16_777_216.0
}
fn noise(r: &Rng) -> f32 {
    rand(r) * 2.0 - 1.0
}

fn to_stereo(mono: &[f32], amp: f32, pan: f32) -> Vec<f32> {
    // Equal-power panning: the middle is as loud as either side.
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    let (l, r) = (angle.cos(), angle.sin());
    let mut out = Vec::with_capacity(mono.len() * 2);
    for &s in mono {
        let s = (s * amp).clamp(-1.0, 1.0);
        out.push(s * l);
        out.push(s * r);
    }
    out
}

/// A cue's sound, mono, at `RATE`. Peaks stay inside -1..1.
pub fn render(cue: &Sound, voice: Voice, other: Voice, r: &Rng) -> Vec<f32> {
    let loud = cue.loud.clamp(0.0, 1.0);
    let size = cue.size.max(1.0);
    let mut out = match cue.kind {
        Kind::Blast => blast(r, cue.size.max(2.0)),
        Kind::Thunder => thunder(r),
        Kind::Hit => {
            let mut s = hit(r, voice, size, loud);
            // What it hit adds its own colour: wood on a steel floor rings a little.
            if other != voice && other != Voice::Silent {
                let floor = hit(r, other, size.max(60.0) * 3.0, loud * 0.6);
                mix(&mut s, &floor, 0.4, 0);
            }
            s
        }
        Kind::Break => smash(r, voice, size, loud),
        Kind::Land => patter(r, voice, (cue.size as usize).clamp(1, 14), loud),
    };
    let gain = 0.25 + 0.75 * loud;
    for s in &mut out {
        *s = (*s * gain).tanh();
    }
    out
}

fn mix(into: &mut Vec<f32>, add: &[f32], amp: f32, at: usize) {
    if into.len() < at + add.len() {
        into.resize(at + add.len(), 0.0);
    }
    for (n, s) in add.iter().enumerate() {
        into[at + n] += s * amp;
    }
}

fn secs(t: f32) -> usize {
    (t * RATE as f32) as usize
}

/// A struck object: a few modes (out-of-tune sines), each fading at its own rate.
fn modes(r: &Rng, f0: f32, ratios: &[f32], amps: &[f32], tau: f32, len: f32, click: f32) -> Vec<f32> {
    let n = secs(len);
    let mut out = vec![0.0; n];
    for (k, (&ratio, &amp)) in ratios.iter().zip(amps).enumerate() {
        let f = f0 * ratio * (1.0 + noise(r) * 0.03);
        if f > RATE as f32 * 0.45 {
            continue;
        }
        let t_k = tau / (1.0 + k as f32 * 0.7);
        let phase = rand(r) * TAU;
        for (i, s) in out.iter_mut().enumerate() {
            let t = i as f32 / RATE as f32;
            *s += amp * (TAU * f * t + phase).sin() * (-t / t_k).exp();
        }
    }
    // The moment of contact: a tiny burst of noise.
    let c = secs(0.004).min(n);
    for s in out.iter_mut().take(c) {
        *s += noise(r) * click;
    }
    fade_in(&mut out);
    out
}

/// Noise through a simple low-pass (`a` 0..1: smaller = duller), fading.
fn thud(r: &Rng, a: f32, tau: f32, len: f32) -> Vec<f32> {
    let mut low = 0.0;
    (0..secs(len))
        .map(|i| {
            let t = i as f32 / RATE as f32;
            low += a * (noise(r) - low);
            low * (-t / tau).exp()
        })
        .collect()
}

/// A falling-pitch sine: the body of a thump or a boom.
fn drop_tone(f_start: f32, f_end: f32, tau: f32, len: f32) -> Vec<f32> {
    let n = secs(len);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let f = f_end + (f_start - f_end) * (-t / (tau * 0.6)).exp();
            phase += TAU * f / RATE as f32;
            phase.sin() * (-t / tau).exp()
        })
        .collect()
}

/// A bubble: a short sine whose pitch rises (that's how bubbles sound).
fn bubble(r: &Rng, f: f32) -> Vec<f32> {
    let len = 0.02 + rand(r) * 0.035;
    let n = secs(len);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let k = i as f32 / n as f32;
            phase += TAU * f * (1.0 + k * 0.8) / RATE as f32;
            phase.sin() * (1.0 - k).powi(2) * (k * 40.0).min(1.0)
        })
        .collect()
}

fn fade_in(s: &mut [f32]) {
    let n = secs(0.0015).min(s.len());
    for (i, v) in s.iter_mut().take(n).enumerate() {
        *v *= i as f32 / n as f32;
    }
}

/// Bigger things ring lower: from a 1-cell pebble to a 10 m girder.
fn pitch_for(size: f32, high: f32, low: f32) -> f32 {
    (high / size.max(1.0).powf(0.32)).clamp(low, high)
}

fn hit(r: &Rng, voice: Voice, size: f32, loud: f32) -> Vec<f32> {
    match voice {
        Voice::Metal => {
            let f0 = pitch_for(size, 2600.0, 70.0) * (0.9 + rand(r) * 0.2);
            let tau = (0.18 + size.powf(0.3) * 0.08).min(1.4) * (0.6 + loud * 0.6);
            let len = (tau * 3.5).min(3.0);
            let mut s = modes(r, f0, &[1.0, 2.76, 5.40, 8.93, 13.34], &[0.45, 0.32, 0.22, 0.13, 0.08], tau, len, 0.25);
            // A dull clank under the ring.
            let clank = thud(r, 0.25, 0.02, 0.08);
            mix(&mut s, &clank, 0.4, 0);
            s
        }
        Voice::Wood => {
            let f0 = pitch_for(size, 900.0, 110.0) * (0.85 + rand(r) * 0.3);
            let mut s = modes(r, f0, &[1.0, 2.31, 3.93], &[0.5, 0.25, 0.12], 0.045, 0.22, 0.2);
            let knock = thud(r, 0.18, 0.025, 0.12);
            mix(&mut s, &knock, 0.5, 0);
            s
        }
        Voice::Stone => {
            let f = pitch_for(size, 260.0, 55.0);
            let mut s = drop_tone(f * 1.4, f, 0.06, 0.3);
            for v in &mut s {
                *v *= 0.5;
            }
            let crunch = thud(r, 0.35, 0.05, 0.3);
            mix(&mut s, &crunch, 0.55, 0);
            // Grit: a few tiny chips.
            for _ in 0..(2.0 + loud * 5.0) as usize {
                let chip = thud(r, 0.6, 0.004, 0.02);
                let at = secs(rand(r) * 0.06);
                mix(&mut s, &chip, 0.25, at);
            }
            s
        }
        Voice::Glass => {
            let f0 = pitch_for(size, 4200.0, 900.0) * (0.85 + rand(r) * 0.3);
            modes(r, f0, &[1.0, 2.32, 4.25, 6.63], &[0.35, 0.25, 0.15, 0.08], 0.22 + loud * 0.15, 0.9, 0.3)
        }
        Voice::Soft => {
            let f = pitch_for(size, 160.0, 45.0);
            let mut s = drop_tone(f * 1.5, f * 0.8, 0.05, 0.2);
            let muff = thud(r, 0.08, 0.03, 0.12);
            mix(&mut s, &muff, 0.4, 0);
            s
        }
        Voice::Earth => {
            let mut s = thud(r, 0.12, 0.05, 0.25);
            for v in &mut s {
                *v *= 1.2;
            }
            for _ in 0..(3.0 + loud * 6.0) as usize {
                let grain = thud(r, 0.5, 0.003, 0.015);
                let at = secs(rand(r) * 0.08);
                mix(&mut s, &grain, 0.2, at);
            }
            s
        }
        Voice::Liquid => splash(r, 2 + (loud * 5.0) as usize, 0.6 + loud * 0.4),
        Voice::Silent => Vec::new(),
    }
}

/// Spray of liquid: hiss and a handful of bubbles.
fn splash(r: &Rng, bubbles: usize, amp: f32) -> Vec<f32> {
    let mut s = thud(r, 0.45, 0.07, 0.35);
    for v in &mut s {
        *v *= 0.45 * amp;
    }
    for _ in 0..bubbles {
        let b = bubble(r, 500.0 + rand(r) * 1400.0);
        let at = secs(rand(r) * 0.18);
        mix(&mut s, &b, 0.3 * amp, at);
    }
    s
}

/// Something breaking apart.
fn smash(r: &Rng, voice: Voice, size: f32, loud: f32) -> Vec<f32> {
    let bits = (3.0 + loud * 6.0 + size.sqrt() * 0.3).min(16.0) as usize;
    let mut s = match voice {
        Voice::Wood => {
            // The crack, then splinters.
            let mut s = thud(r, 0.7, 0.012, 0.05);
            for v in &mut s {
                *v *= 1.4;
            }
            s
        }
        Voice::Glass => modes(r, 3000.0 + rand(r) * 1500.0, &[1.0, 2.32, 4.25], &[0.3, 0.2, 0.1], 0.15, 0.4, 0.5),
        Voice::Metal => hit(r, Voice::Metal, size * 0.3, loud),
        Voice::Liquid => splash(r, 6, 1.0),
        _ => {
            let f = pitch_for(size, 200.0, 45.0);
            drop_tone(f * 1.3, f, 0.12, 0.4)
        }
    };
    let spread = if voice == Voice::Glass { 0.45 } else { 0.3 };
    for _ in 0..bits {
        let at = secs(rand(r) * spread);
        let piece = match voice {
            Voice::Glass => modes(r, 2500.0 + rand(r) * 4000.0, &[1.0, 2.7], &[0.2, 0.1], 0.06, 0.15, 0.2),
            Voice::Wood => {
                let mut c = thud(r, 0.6, 0.006, 0.03);
                let k = modes(r, 400.0 + rand(r) * 700.0, &[1.0, 2.3], &[0.15, 0.06], 0.02, 0.05, 0.0);
                mix(&mut c, &k, 1.0, 0);
                c
            }
            Voice::Metal => modes(r, 1200.0 + rand(r) * 2500.0, &[1.0, 2.76], &[0.15, 0.08], 0.12, 0.3, 0.1),
            Voice::Soft => thud(r, 0.1, 0.02, 0.06),
            _ => thud(r, 0.4 + rand(r) * 0.3, 0.01, 0.04),
        };
        mix(&mut s, &piece, 0.5 + rand(r) * 0.5, at);
    }
    s
}

/// Many small landings at once (spray, gravel, hailstones).
fn patter(r: &Rng, voice: Voice, n: usize, loud: f32) -> Vec<f32> {
    let mut s = Vec::new();
    for k in 0..n {
        let at = secs(rand(r) * 0.06 * (n as f32).sqrt());
        let one = match voice {
            Voice::Liquid => {
                let mut b = bubble(r, 700.0 + rand(r) * 1800.0);
                let hiss = thud(r, 0.6, 0.01, 0.03);
                mix(&mut b, &hiss, 0.3, 0);
                b
            }
            Voice::Glass => modes(r, 2500.0 + rand(r) * 2500.0, &[1.0, 2.32], &[0.25, 0.1], 0.03, 0.08, 0.3),
            Voice::Metal => modes(r, 1500.0 + rand(r) * 2000.0, &[1.0, 2.76], &[0.2, 0.1], 0.08, 0.2, 0.15),
            Voice::Wood => modes(r, 500.0 + rand(r) * 500.0, &[1.0, 2.31], &[0.25, 0.1], 0.02, 0.06, 0.2),
            Voice::Soft => thud(r, 0.12, 0.015, 0.05),
            Voice::Silent => Vec::new(),
            _ => thud(r, 0.35 + rand(r) * 0.3, 0.006, 0.03),
        };
        let amp = (0.5 + rand(r) * 0.5) * if k == 0 { 1.0 } else { 0.7 } * (0.4 + loud);
        mix(&mut s, &one, amp, at);
    }
    s
}

/// An explosion: a crack, a deep drop and a long rough roar.
fn blast(r: &Rng, size: f32) -> Vec<f32> {
    let len = (1.2 + size / 18.0).min(3.5);
    let deep = (60.0 - size.min(40.0) * 0.6).max(30.0);
    let mut s = drop_tone(deep * 2.2, deep * 0.6, 0.35 + size / 80.0, len);
    for v in &mut s {
        *v *= 0.8;
    }
    let roar = thud(r, 0.06, 0.3 + size / 60.0, len);
    mix(&mut s, &roar, 1.6, 0);
    let crack = thud(r, 0.8, 0.015, 0.08);
    mix(&mut s, &crack, 0.9, 0);
    // Debris rattling down afterwards.
    for _ in 0..(size.min(30.0) as usize / 3 + 3) {
        let bit = thud(r, 0.4, 0.008, 0.03);
        let at = secs(0.15 + rand(r) * (len - 0.3).max(0.1));
        mix(&mut s, &bit, 0.12, at);
    }
    fade_in(&mut s);
    s
}

/// Thunder: a sharp tearing crack, then a long rolling rumble.
fn thunder(r: &Rng) -> Vec<f32> {
    let len = 3.2 + rand(r) * 2.0;
    let n = secs(len);
    let mut s = vec![0.0; n];
    let mut low = 0.0;
    let mut slow = 0.0;
    let mut roll = 0.5;
    for (i, v) in s.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        low += 0.015 * (noise(r) - low);
        // The rumble swells and falls as the sound arrives from different parts of the bolt.
        if i % 512 == 0 {
            roll = 0.3 + rand(r) * 0.9;
        }
        slow += 0.0004 * (roll - slow);
        let env = (t * 8.0).min(1.0) * (-t / (len * 0.35)).exp();
        *v = low * 5.0 * slow * env;
    }
    // The crack: bright, crackling.
    let mut crackle = Vec::with_capacity(secs(0.25));
    for i in 0..secs(0.25) {
        let t = i as f32 / RATE as f32;
        let n = noise(r);
        crackle.push(if rand(r) < 0.3 { n } else { n * 0.2 } * (-t / 0.07).exp());
    }
    mix(&mut s, &crackle, 0.7, 0);
    fade_in(&mut s);
    s
}

/// The endless stream: fire, rain, hail, wind.
struct Ambience {
    levels: Arc<Levels>,
    r: Rng,
    left: bool,
    // Smoothed levels.
    fire: f32,
    rain: f32,
    hail: f32,
    wind: f32,
    volume: f32,
    pan: f32,
    // Filters and the sounds in progress.
    roar: f32,
    hiss_lo: (f32, f32),
    wind_bp: (f32, f32),
    wind_f: f32,
    wind_target: f32,
    gust: f32,
    pop: (f32, f32, f32), // (amplitude, decay per sample, brightness filter)
    pop_lo: f32,
    tick: (f32, f32, f32), // (amplitude, phase, freq)
    out: (f32, f32),
    n: u64,
}

impl Ambience {
    fn new(levels: Arc<Levels>) -> Self {
        Ambience {
            levels,
            r: Rng::new(0x1357_9BDF),
            left: true,
            fire: 0.0,
            rain: 0.0,
            hail: 0.0,
            wind: 0.0,
            volume: 0.0,
            pan: 0.0,
            roar: 0.0,
            hiss_lo: (0.0, 0.0),
            wind_bp: (0.0, 0.0),
            wind_f: 400.0,
            wind_target: 400.0,
            gust: 0.5,
            pop: (0.0, 0.0, 0.5),
            pop_lo: 0.0,
            tick: (0.0, 0.0, 3000.0),
            out: (0.0, 0.0),
            n: 0,
        }
    }

    fn frame(&mut self) -> (f32, f32) {
        let r = &self.r;
        if self.n % 256 == 0 {
            let l = &self.levels;
            let k = 0.06;
            self.fire += (get(&l.fire) - self.fire) * k;
            self.rain += (get(&l.rain) - self.rain) * k;
            self.hail += (get(&l.hail) - self.hail) * k;
            self.wind += (get(&l.wind) - self.wind) * k * 0.5;
            self.volume += (get(&l.volume) - self.volume) * k;
            self.pan += (get(&l.fire_pan) - self.pan) * k;
            if rand(r) < 0.01 {
                self.wind_target = 250.0 + rand(r) * 550.0;
            }
            self.gust += ((0.4 + rand(r) * 0.8) - self.gust) * 0.01;
        }
        self.n += 1;
        if self.volume < 0.001 && self.fire + self.rain + self.hail + self.wind < 0.001 {
            return (0.0, 0.0);
        }
        let (mut l, mut rr) = (0.0, 0.0);
        // Fire: a soft roar and random pops and snaps.
        if self.fire > 0.001 {
            self.roar += 0.02 * (noise(r) - self.roar);
            let roar = self.roar * self.fire * 0.5;
            if self.pop.0 < 0.01 && rand(r) < self.fire * 0.0012 {
                let big = rand(r) < 0.12;
                self.pop = (if big { 0.9 } else { 0.25 + rand(r) * 0.4 }, if big { 0.9985 } else { 0.996 }, 0.3 + rand(r) * 0.6);
            }
            self.pop_lo += self.pop.2 * (noise(r) - self.pop_lo);
            let pop = self.pop_lo * self.pop.0;
            self.pop.0 *= self.pop.1;
            let s = (roar + pop * self.fire.sqrt()) * 0.6;
            l += s * (1.0 - self.pan).sqrt() * std::f32::consts::FRAC_1_SQRT_2;
            rr += s * (1.0 + self.pan).sqrt() * std::f32::consts::FRAC_1_SQRT_2;
        }
        // Rain: a high hiss (a little different in each ear) and drips.
        if self.rain > 0.001 {
            let (a, b) = (noise(r), noise(r));
            self.hiss_lo.0 += 0.3 * (a - self.hiss_lo.0);
            self.hiss_lo.1 += 0.3 * (b - self.hiss_lo.1);
            l += (a - self.hiss_lo.0) * self.rain * 0.12;
            rr += (b - self.hiss_lo.1) * self.rain * 0.12;
        }
        // Drips and hailstones: tiny pings.
        if self.tick.0 < 0.01 {
            let chance = self.rain * 0.0012 + self.hail * 0.004;
            if rand(r) < chance {
                let hail = rand(r) * (self.rain + self.hail) < self.hail;
                self.tick = (if hail { 0.35 } else { 0.12 } * (0.4 + rand(r) * 0.6), 0.0, if hail { 1800.0 + rand(r) * 2500.0 } else { 2500.0 + rand(r) * 3500.0 });
            }
        }
        if self.tick.0 > 0.0005 {
            self.tick.1 += TAU * self.tick.2 / RATE as f32;
            let s = self.tick.1.sin() * self.tick.0;
            self.tick.0 *= 0.993;
            let side = (self.tick.2 as u32 % 2) as f32;
            l += s * (0.4 + side * 0.6);
            rr += s * (1.0 - side * 0.6);
        }
        // Wind: noise through a slowly wandering band-pass.
        if self.wind > 0.001 {
            self.wind_f += (self.wind_target - self.wind_f) * 0.00005;
            let f = (TAU * self.wind_f / RATE as f32).min(1.0);
            let q = 0.08;
            let x = noise(r);
            // State-variable filter.
            let (low, band) = self.wind_bp;
            let low = low + f * band;
            let high = x - low - q * band;
            let band = f * high + band;
            self.wind_bp = (low, band);
            let s = band * self.wind * self.gust * 0.35;
            l += s;
            rr += s * 0.9;
        }
        let v = self.volume;
        self.out = ((l * v).tanh(), (rr * v).tanh());
        self.out
    }
}

impl Iterator for Ambience {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let s = if self.left {
            self.frame();
            self.out.0
        } else {
            self.out.1
        };
        self.left = !self.left;
        Some(s)
    }
}

impl Source for Ambience {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_is_finite_and_never_clips() {
        let r = Rng::new(7);
        for kind in [Kind::Hit, Kind::Break, Kind::Blast, Kind::Thunder, Kind::Land] {
            for voice in Voice::ALL {
                for (loud, size) in [(0.05, 1.0), (0.5, 80.0), (1.0, 5000.0), (1.0, 1.0e6)] {
                    let cue = Sound { kind, x: 0.0, y: 0.0, mat: 0, other: 0, loud, size };
                    let s = render(&cue, voice, Voice::Stone, &r);
                    assert!(s.iter().all(|v| v.is_finite() && v.abs() <= 1.0), "{kind:?} {voice:?}");
                    assert!(s.len() < secs(6.0), "{kind:?} {voice:?} too long");
                    if voice != Voice::Silent || matches!(kind, Kind::Blast | Kind::Thunder) {
                        assert!(s.iter().any(|v| v.abs() > 0.01), "{kind:?} {voice:?} is silent");
                    }
                }
            }
        }
    }

    #[test]
    fn bigger_metal_rings_lower() {
        assert!(pitch_for(1.0, 2600.0, 70.0) > pitch_for(100.0, 2600.0, 70.0));
        assert!(pitch_for(100.0, 2600.0, 70.0) > pitch_for(10_000.0, 2600.0, 70.0));
    }

    #[test]
    fn the_endless_stream_stays_in_range() {
        let levels = Arc::new(Levels::default());
        for l in [&levels.fire, &levels.rain, &levels.hail, &levels.wind, &levels.volume] {
            set(l, 1.0);
        }
        let mut a = Ambience::new(levels);
        for _ in 0..RATE * 4 {
            let s = a.next().unwrap();
            assert!(s.is_finite() && s.abs() <= 1.0);
        }
    }
}
