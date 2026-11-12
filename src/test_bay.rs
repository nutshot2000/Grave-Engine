//! Saved repeatable destructive tests; every run instantiates a fresh asset copy.
use crate::{Materials, Sim, asset::Asset, world::Blast};
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TestKind {
    Drop,
    Impact,
    Burn,
    Explosion,
    Snow,
}
impl TestKind {
    pub const ALL: [Self; 5] = [
        Self::Drop,
        Self::Impact,
        Self::Burn,
        Self::Explosion,
        Self::Snow,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Drop => "Drop",
            Self::Impact => "Impact",
            Self::Burn => "Burn",
            Self::Explosion => "Explosion",
            Self::Snow => "Snow",
        }
    }
}
#[derive(Clone)]
pub struct Recipe {
    pub asset: String,
    pub name: String,
    pub kind: TestKind,
    pub speed: f32,
    pub heat: f32,
    pub frames: u32,
}
impl Default for Recipe {
    fn default() -> Self {
        Self {
            asset: "fuel_crate".into(),
            name: "my_test".into(),
            kind: TestKind::Burn,
            speed: 40.,
            heat: 900.,
            frames: 600,
        }
    }
}
impl Recipe {
    pub fn text(&self) -> String {
        format!(
            "test 1\nasset {}\nname {}\nkind {}\nspeed {}\nheat {}\nframes {}\n",
            self.asset,
            self.name,
            self.kind.name(),
            self.speed,
            self.heat,
            self.frames
        )
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut r = Self::default();
        let mut version = false;
        for line in text.lines() {
            let w: Vec<_> = line.split_whitespace().collect();
            if w.is_empty() {
                continue;
            }
            let val = w.get(1).ok_or("Missing test value")?;
            match w[0] {
                "test" => {
                    if *val != "1" {
                        return Err("Unsupported test format".into());
                    }
                    version = true;
                }
                "asset" => r.asset = val.to_string(),
                "name" => r.name = val.to_string(),
                "kind" => {
                    r.kind = TestKind::ALL
                        .into_iter()
                        .find(|k| k.name() == *val)
                        .ok_or("Unknown test kind")?
                }
                "speed" => r.speed = val.parse().map_err(|_| "Invalid speed")?,
                "heat" => r.heat = val.parse().map_err(|_| "Invalid heat")?,
                "frames" => r.frames = val.parse().map_err(|_| "Invalid frames")?,
                _ => return Err("Unknown test command".into()),
            }
        }
        if !version
            || !crate::asset::valid_name(&r.name)
            || !crate::asset::valid_name(&r.asset)
            || !r.speed.is_finite()
            || !(0.0..=120.).contains(&r.speed)
            || !r.heat.is_finite()
            || !(0.0..=3000.).contains(&r.heat)
            || !(1..=3600).contains(&r.frames)
        {
            return Err("Test values outside their limits".into());
        }
        Ok(r)
    }
    pub fn start(&self, a: &Asset, m: &Materials) -> Result<Sim, String> {
        let mut s = Sim::empty(240, 180, m);
        let stone = m.id("stone").ok_or("Missing stone")?;
        for y in 160..180 {
            for x in 0..240 {
                s.world.set(m, x, y, stone);
            }
        }
        if self.kind == TestKind::Impact {
            for y in 0..160 {
                for x in 190..196 {
                    s.world.set(m, x, y, stone);
                }
            }
        }
        let x = if self.kind == TestKind::Impact {
            85.
        } else {
            120.
        };
        let y = if self.kind == TestKind::Drop {
            (127. - a.h as f32).max(1.)
        } else {
            157. - a.h as f32
        };
        a.instantiate(&mut s, m, x, y + a.pivot.1)?;
        if self.kind == TestKind::Impact {
            for _ in 0..10 {
                s.step(m);
            }
            for z in 0..=s.layers.len() {
                let p = crate::assemblies::plane_mut(&mut s, z);
                for id in p.physics.ids() {
                    p.physics.set_velocity(id, self.speed * 10., 0., 0.);
                }
            }
        }
        if self.kind == TestKind::Burn {
            s.world.heat_circle(x as i32, 145, 16, self.heat);
        }
        if self.kind == TestKind::Explosion {
            s.world.blasts.push(Blast {
                x: x - 22.,
                y: 140.,
                size: 24.,
                power: 600.,
                fire: m.id("fire"),
            });
            s.physics.update(&mut s.world, m);
        }
        if self.kind == TestKind::Snow {
            s.set_outside_temperature(-8., m, true)?;
            s.weather.snow = 0.8;
        }
        Ok(s)
    }
}
