//! Headless mode: load a scene, simulate without a window, save screenshots.
//! Built for AI agents checking their own work.

use crate::material::Materials;
use crate::render::{Renderer, View};
use crate::world::World;
use std::path::{Path, PathBuf};

pub struct Options {
    pub scene: PathBuf,
    pub materials: PathBuf,
    pub frames: u32,
    pub shot: Option<PathBuf>,
    /// Also save a shot every N frames (out_00100.png, ...).
    pub every: Option<u32>,
    pub scale: u32,
    pub heat: bool,
    /// Hold the drive controls: 1 = full ahead, -1 = full reverse.
    pub throttle: f32,
    /// Hold the player's walk controls (-1 left, 1 right) and jump.
    pub walk: f32,
    pub jump: bool,
}

/// Write an RGBA image as PNG, scaled up by a whole number.
pub fn write_png(path: &Path, w: usize, h: usize, rgba: &[u8], scale: u32) -> Result<(), String> {
    let s = scale.max(1) as usize;
    let (sw, sh) = (w * s, h * s);
    let mut data = vec![0u8; sw * sh * 4];
    for y in 0..sh {
        for x in 0..sw {
            let src = ((y / s) * w + x / s) * 4;
            let dst = (y * sw + x) * 4;
            data[dst..dst + 4].copy_from_slice(&rgba[src..src + 4]);
        }
    }
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
    }
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), sw as u32, sh as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&data).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn shot(world: &World, mats: &Materials, r: &mut Renderer, path: &Path, scale: u32, heat: bool) -> Result<(), String> {
    let rgba = r.draw(world, mats, View { heat });
    write_png(path, world.w, world.h, rgba, scale)
}

fn numbered(path: &Path, frame: u32) -> PathBuf {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_else(|| "png".into());
    path.with_file_name(format!("{stem}_{frame:05}.{ext}"))
}

/// Run and print a short report. Returns an exit code
/// (0 = fine, 3 = ran but there were problems in the files).
pub fn run(o: &Options) -> i32 {
    let mats = Materials::load_dir(&o.materials);
    for p in &mats.problems {
        eprintln!("material problem: {p}");
    }
    let text = match std::fs::read_to_string(&o.scene) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("can't read scene {}: {e}", o.scene.display());
            return 2;
        }
    };
    let loaded = crate::scene::load(&text, &mats, o.scene.parent());
    let (mut sim, problems) = loaded.into_sim(&mats);
    for p in &problems {
        eprintln!("scene problem: {p}");
    }
    let objects_at_start = sim.physics.body_count();
    let mut r = Renderer::default();
    let shot_path = o.shot.clone().unwrap_or_else(|| PathBuf::from("shots/out.png"));
    let start = std::time::Instant::now();
    sim.physics.set_throttle(o.throttle);
    sim.physics.set_controls(grave_engine_controls(o.walk, o.jump));
    for f in 1..=o.frames {
        sim.step(&mats);
        if let Some(every) = o.every.filter(|e| *e > 0) {
            if f % every == 0 && f != o.frames {
                if let Err(e) = shot(&sim.world, &mats, &mut r, &numbered(&shot_path, f), o.scale, o.heat) {
                    eprintln!("screenshot failed: {e}");
                }
            }
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    let specs = sim.physics.export(&mut sim.world);
    let world = &sim.world;
    if let Err(e) = shot(world, &mats, &mut r, &shot_path, o.scale, o.heat) {
        eprintln!("screenshot failed: {e}");
        return 1;
    }
    println!(
        "Simulated {} frames of {}x{} in {:.2}s ({:.2} ms/frame). Screenshot: {}",
        o.frames,
        world.w,
        world.h,
        elapsed,
        elapsed * 1000.0 / o.frames.max(1) as f64,
        shot_path.display()
    );
    let (mut tmin, mut tmax) = (f32::MAX, f32::MIN);
    for t in &world.temp {
        tmin = tmin.min(*t);
        tmax = tmax.max(*t);
    }
    println!("Temperature range: {tmin:.0}°C to {tmax:.0}°C");
    println!("Objects: {} at the start, {} now", objects_at_start, sim.physics.body_count());
    for (n, a) in sim.physics.actor_status().iter().enumerate() {
        let state = if a.dead {
            "dead".to_string()
        } else if a.swimming {
            "swimming".into()
        } else if a.grounded {
            "standing".into()
        } else {
            "in the air".into()
        };
        println!(
            "Player {}: at x={:.0} y={:.0}, {state}, health {:.0}/100, breath {:.1}s, body {:.0}°C{}",
            n + 1,
            a.x,
            a.y,
            a.health,
            a.breath,
            a.temp,
            a.hurt_by.map(|h| format!(" (just {h})")).unwrap_or_default()
        );
    }
    for b in specs.iter().take(40) {
        let mut names: Vec<(String, usize)> = Vec::new();
        for c in b.cells.iter().flatten() {
            let n = &mats.get(c.mat).name;
            match names.iter_mut().find(|e| &e.0 == n) {
                Some(e) => e.1 += 1,
                None => names.push((n.clone(), 1)),
            }
        }
        let what: Vec<String> = names.iter().map(|(n, c)| format!("{c} {n}")).collect();
        let speed = (b.vx * b.vx + b.vy * b.vy).sqrt() * crate::physics::CELL;
        println!(
            "  object at ({:.0}, {:.0}), turned {:.0}°, moving {:.1} m/s: {}",
            b.x,
            b.y,
            b.angle.to_degrees(),
            speed,
            what.join(", ")
        );
    }
    if specs.len() > 40 {
        println!("  ...and {} more", specs.len() - 40);
    }
    println!("Cells by material:");
    for (name, count, burning) in world.census(&mats) {
        if burning > 0 {
            println!("  {name:<12} {count:>7}  ({burning} burning)");
        } else {
            println!("  {name:<12} {count:>7}");
        }
    }
    if !world.flying.is_empty() {
        println!("Flying through the air: {} cells", world.flying.len());
    }
    if problems.is_empty() && mats.problems.is_empty() { 0 } else { 3 }
}

fn grave_engine_controls(walk: f32, jump: bool) -> crate::physics::Controls {
    crate::physics::Controls { walk, jump }
}
