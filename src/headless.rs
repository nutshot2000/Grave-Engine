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
    /// Screenshot only this many cells across around the player (0 = everything).
    pub player_view: usize,
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

/// A screenshot with characters' pictures drawn on top (sharp, at the
/// screenshot's resolution), optionally only a window `(x, y, w, h)` of the
/// world in cells.
pub fn shot_with_actors(
    world: &World,
    mats: &Materials,
    r: &mut Renderer,
    actors: &[crate::physics::ActorStatus],
    window: Option<(usize, usize, usize, usize)>,
    path: &Path,
    scale: u32,
    heat: bool,
) -> Result<(), String> {
    let rgba = r.draw(world, mats, View { heat });
    let (x0, y0, cw, ch) = window.unwrap_or((0, 0, world.w, world.h));
    let s = scale.max(1) as usize;
    let (ow, oh) = (cw * s, ch * s);
    let mut out = vec![0u8; ow * oh * 4];
    for oy in 0..oh {
        for ox in 0..ow {
            let (wx, wy) = ((x0 + ox / s).min(world.w - 1), (y0 + oy / s).min(world.h - 1));
            let src = (wy * world.w + wx) * 4;
            out[(oy * ow + ox) * 4..(oy * ow + ox) * 4 + 4].copy_from_slice(&rgba[src..src + 4]);
        }
    }
    let body_h = crate::physics::actor_height();
    for a in actors {
        let Some(look) = &a.look else { continue };
        let (frame, _) = look.frame(a.anim, a.anim_time);
        let cpp = look.cells_per_px(body_h);
        let feet = a.y + body_h as f32 / 2.0;
        let left = a.x - frame.w as f32 * cpp / 2.0;
        let top = feet - look.feet * cpp;
        let mirror = (a.facing < 0.0) == look.faces_right;
        // Output pixels the picture covers.
        let to_out = |c: f32, o: usize| (c - o as f32) * s as f32;
        let (px0, px1) = (to_out(left, x0).floor().max(0.0) as usize, (to_out(left + frame.w as f32 * cpp, x0).ceil().max(0.0) as usize).min(ow));
        let (py0, py1) = (to_out(top, y0).floor().max(0.0) as usize, (to_out(top + frame.h as f32 * cpp, y0).ceil().max(0.0) as usize).min(oh));
        for oy in py0..py1 {
            for ox in px0..px1 {
                let wx = x0 as f32 + (ox as f32 + 0.5) / s as f32;
                let wy = y0 as f32 + (oy as f32 + 0.5) / s as f32;
                let mut fx = (wx - left) / cpp;
                let fy = (wy - top) / cpp;
                if mirror {
                    fx = frame.w as f32 - fx;
                }
                let c = sample(frame, fx, fy);
                let a = c[3] / 255.0;
                if a <= 0.0 {
                    continue;
                }
                let o = (oy * ow + ox) * 4;
                for k in 0..3 {
                    out[o + k] = (c[k] * a + out[o + k] as f32 * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
    write_png(path, ow, oh, &out, 1)
}

/// Bilinear sample of a picture (RGBA, 0..255).
fn sample(f: &crate::look::Frame, x: f32, y: f32) -> [f32; 4] {
    let (x, y) = (x - 0.5, y - 0.5);
    let (ix, iy) = (x.floor() as i64, y.floor() as i64);
    let (tx, ty) = (x - ix as f32, y - iy as f32);
    let px = |x: i64, y: i64| -> [f32; 4] {
        if x < 0 || y < 0 || x >= f.w as i64 || y >= f.h as i64 {
            return [0.0; 4];
        }
        let o = (y as usize * f.w + x as usize) * 4;
        let a = f.rgba[o + 3] as f32;
        // Premultiplied, so transparent edges don't go dark.
        [f.rgba[o] as f32 * a / 255.0, f.rgba[o + 1] as f32 * a / 255.0, f.rgba[o + 2] as f32 * a / 255.0, a]
    };
    let (a, b, c, d) = (px(ix, iy), px(ix + 1, iy), px(ix, iy + 1), px(ix + 1, iy + 1));
    let mut out = [0.0; 4];
    for k in 0..4 {
        out[k] = (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d[k] * tx) * ty;
    }
    if out[3] > 0.0 {
        for k in 0..3 {
            out[k] *= 255.0 / out[3];
        }
    }
    out
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
                let actors = sim.physics.actor_status();
                let window = view_window(&sim.world, &actors, o.player_view);
                r.light(&sim);
                if let Err(e) = shot_with_actors(&sim.world, &mats, &mut r, &actors, window, &numbered(&shot_path, f), o.scale, o.heat) {
                    eprintln!("screenshot failed: {e}");
                }
            }
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    r.light(&sim);
    let specs = sim.physics.export(&mut sim.world);
    let world = &sim.world;
    let actors = sim.physics.actor_status();
    let window = view_window(world, &actors, o.player_view);
    if let Err(e) = shot_with_actors(world, &mats, &mut r, &actors, window, &shot_path, o.scale, o.heat) {
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

/// A window `cells` wide (16:9) around the first player, inside the world.
fn view_window(world: &World, actors: &[crate::physics::ActorStatus], cells: usize) -> Option<(usize, usize, usize, usize)> {
    let a = actors.first()?;
    if cells == 0 {
        return None;
    }
    let w = cells.min(world.w);
    let h = (cells * 9 / 16).min(world.h);
    let x = (a.x - w as f32 / 2.0).clamp(0.0, (world.w - w) as f32) as usize;
    let y = (a.y - h as f32 / 2.0).clamp(0.0, (world.h - h) as f32) as usize;
    Some((x, y, w, h))
}
