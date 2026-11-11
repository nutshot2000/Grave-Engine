//! Original synthesised soundtrack and bounded one-shot effects. No external media.
use rodio::{OutputStream, OutputStreamBuilder, Sink, Source, buffer::SamplesBuffer};
use std::f32::consts::TAU;
const RATE: u32 = 22050;
pub struct Audio {
    _stream: OutputStream,
    music: Sink,
    effects: Vec<Vec<f32>>,
    pub muted: bool,
}
fn tone(kind: usize) -> Vec<f32> {
    let duration = match kind {
        0 => 0.07,
        1 => 0.10,
        2 => 0.65,
        3 => 0.38,
        4 => 0.23,
        _ => 0.22,
    };
    let mut seed = 137u32;
    (0..(duration * RATE as f32) as usize)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let u = t / duration;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise = (seed >> 8) as f32 / 16777216. * 2. - 1.;
            let e = (1. - u).powi(2) * (t * 450.).min(1.);
            let v = match kind {
                0 => (TAU * (1500. * t - 5500. * t * t)).sin() * 0.16,
                1 => noise * 0.18,
                2 => noise * 0.4 + (TAU * (75. * t - 35. * t * t)).sin() * 0.3,
                3 => {
                    let f = if u < 0.33 {
                        523.25
                    } else if u < 0.66 {
                        659.25
                    } else {
                        783.99
                    };
                    (TAU * f * t).sin() * 0.25
                }
                4 => (TAU * 360. * t).sin() * 0.2,
                _ => noise * 0.12 + (TAU * (180. * t + 900. * t * t)).sin() * 0.18,
            };
            v * e
        })
        .collect()
}
fn music() -> Vec<f32> {
    // Eight-bar C-minor synth ostinato with a restrained kick, hats and snare.
    let beat = 0.5;
    let length = beat * 32.;
    let bass = [65.406, 51.913, 58.27, 48.999];
    let notes = [
        261.626, 311.127, 391.995, 523.251, 466.164, 391.995, 311.127, 233.082,
    ];
    let mut seed = 27u32;
    (0..(length * RATE as f32) as usize)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let b = (t / beat) as usize;
            let bt = t % beat;
            let chord = (b / 8) % 4;
            let step = (t / (beat / 2.)) as usize;
            let st = t % (beat / 2.);
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let n = (seed >> 8) as f32 / 16777216. * 2. - 1.;
            let low = (TAU * bass[chord] * t).sin() * 0.07 * (-bt * 4.).exp();
            let arp = (TAU * notes[step % 8] * (bass[chord] / 65.406) * t).sin()
                * 0.025
                * (-st * 14.).exp();
            let kick =
                (TAU * (52. * bt + 3. * (1. - (-bt * 25.).exp()))).sin() * (-bt * 20.).exp() * 0.15;
            let hat = n * (-st * 100.).exp() * 0.035;
            let snare = if b % 2 == 1 {
                n * (-bt * 28.).exp() * 0.055
            } else {
                0.
            };
            (low + arp + kick + hat + snare) * (t * 3.).min(1.)
        })
        .collect()
}
impl Audio {
    pub fn open() -> Option<Self> {
        let mut stream = OutputStreamBuilder::open_default_stream().ok()?;
        stream.log_on_drop(false);
        let music = Sink::connect_new(stream.mixer());
        music.append(SamplesBuffer::new(1, RATE, self::music()).repeat_infinite());
        music.pause();
        Some(Self {
            _stream: stream,
            music,
            effects: (0..6).map(tone).collect(),
            muted: false,
        })
    }
    pub fn play(&self, kind: usize) {
        if !self.muted {
            self._stream.mixer().add(SamplesBuffer::new(
                1,
                RATE,
                self.effects[kind.min(5)].clone(),
            ));
        }
    }
    pub fn update(&self, active: bool) {
        self.music.set_volume(if self.muted { 0.0 } else { 0.75 });
        if active {
            self.music.play();
        } else {
            self.music.pause();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_audio_is_finite_and_bounded() {
        for k in 0..6 {
            let b = tone(k);
            assert!(!b.is_empty() && b.iter().all(|s| s.is_finite() && s.abs() <= 1.));
        }
        let m = music();
        assert!(m.iter().all(|s| s.is_finite() && s.abs() < 1.));
    }
}
