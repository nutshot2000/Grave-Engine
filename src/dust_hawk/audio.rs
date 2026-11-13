//! Original restrained rotor ambience and short weapon effects. No music or media files.
use rodio::{OutputStream, OutputStreamBuilder, Sink, Source, buffer::SamplesBuffer};
use std::f32::consts::TAU;
const RATE: u32 = 22050;
pub struct Audio {
    stream: OutputStream,
    rotor: Sink,
    effects: Vec<Vec<f32>>,
}
impl Audio {
    pub fn open() -> Option<Self> {
        let mut stream = OutputStreamBuilder::open_default_stream().ok()?;
        stream.log_on_drop(false);
        let rotor = Sink::connect_new(stream.mixer());
        let samples = (0..RATE * 2)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                ((TAU * 48. * t).sin() * 0.45 + (TAU * 96. * t).sin() * 0.15)
                    * (0.6 + 0.4 * (TAU * 12. * t).sin().powi(8))
                    * 0.055
            })
            .collect::<Vec<_>>();
        rotor.append(SamplesBuffer::new(1, RATE, samples).repeat_infinite());
        rotor.pause();
        let effects = (0..4)
            .map(|kind| {
                let duration = [0.055, 0.22, 0.6, 0.14][kind];
                let mut seed = 13u32;
                let mut low = 0.;
                (0..(RATE as f32 * duration) as usize)
                    .map(|i| {
                        let t = i as f32 / RATE as f32;
                        let f = t / duration;
                        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                        let n = (seed >> 8) as f32 / 16777216. * 2. - 1.;
                        low += 0.18 * (n - low);
                        let value = match kind {
                            0 => n * 0.14 + (TAU * 120. * t).sin() * 0.06,
                            1 => n * 0.08 + (TAU * (120. * t - 140. * t * t)).sin() * 0.08,
                            2 => low * 0.65 + (TAU * (48. * t - 25. * t * t)).sin() * 0.15,
                            _ => (TAU * 700. * t).sin() * 0.06,
                        };
                        value * (1. - f).powi(2) * (t * 600.).min(1.)
                    })
                    .collect()
            })
            .collect();
        Some(Self {
            stream,
            rotor,
            effects,
        })
    }
    pub fn ambience(&self, on: bool, speed: f32) {
        self.rotor.set_speed(1. + speed * 0.0018);
        if on {
            self.rotor.play()
        } else {
            self.rotor.pause()
        }
    }
    pub fn effect(&self, kind: usize) {
        self.stream.mixer().add(SamplesBuffer::new(
            1,
            RATE,
            self.effects[kind.min(3)].clone(),
        ));
    }
}
