//! Inspect a look and replay an animation graph through the same animator as the editor.
use grave_engine::{animation::{Animator, Graph}, look};
use std::{env, fs, path::Path, sync::Arc};

const USAGE: &str = "animation-doctor LOOK [--graph FILE] [--script FILE] [--shot PNG]\nScript commands: set PARAM VALUE, trigger NAME, step FRAMES, expect STATE. One command per line.";

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let name = args.next().ok_or(USAGE)?;
    if name == "--help" || name == "-h" { println!("{USAGE}"); return Ok(()); }
    let (mut graph_path, mut script, mut shot) = (None, None, None);
    while let Some(option) = args.next() {
        let target = match option.as_str() {
            "--graph" => &mut graph_path, "--script" => &mut script, "--shot" => &mut shot,
            _ => return Err(format!("Unknown option {option}\n{USAGE}")),
        };
        *target = Some(args.next().ok_or_else(|| format!("{option} needs a file"))?);
    }
    let look = look::find(&name, None, grave_engine::physics::actor_height())?;
    let graph = match graph_path {
        Some(file) => Arc::new(Graph::parse(&fs::read_to_string(&file).map_err(|e| format!("{file}: {e}"))?)
            .map_err(|e| format!("{file}: {e}"))?),
        None => look.graph.clone(),
    };
    println!("Look: {}\nClips:", look.name);
    for (name, clip) in &look.anims { println!("  {name}: {} frames at {} fps", clip.frames.len(), clip.fps); }
    for state in &graph.states {
        if !look.anims.contains_key(&state.clip) { return Err(format!("State {} needs missing clip {}", state.name, state.clip)); }
    }
    look.validate_blends(&graph)?;
    let mut anim = Animator::new(graph)?;
    println!("Parameters: {:?}\nTriggers: {:?}\nFrame 0: {}", anim.parameters, anim.graph.triggers, anim.state().name);
    let mut frame = 0usize;
    if let Some(file) = script {
        let spec = fs::read_to_string(&file).map_err(|e| format!("{file}: {e}"))?;
        for (i, line) in spec.lines().enumerate() {
            let words: Vec<_> = line.split('#').next().unwrap_or("").split_whitespace().collect();
            let location = || format!("{file} line {} (frame {frame})", i + 1);
            match words.as_slice() {
                [] => (),
                ["set", param, value] => {
                    let value = value.parse::<f32>().ok().filter(|n| n.is_finite()).ok_or_else(|| format!("{}: invalid number", location()))?;
                    if !anim.set(param, value) { return Err(format!("{}: unknown parameter {param}", location())); }
                }
                ["trigger", name] => if !anim.trigger(name) { return Err(format!("{}: unknown trigger {name}", location())); },
                ["step", count] => {
                    let frames = count.parse::<usize>().ok().filter(|n| *n <= 36000 - frame)
                        .ok_or_else(|| format!("{}: expected a frame count within the 36000-frame run limit", location()))?;
                    for _ in 0..frames {
                        let previous = anim.state().name.clone();
                        let events = anim.advance(1. / 60., |c| look.duration(c));
                        frame += 1;
                        if anim.state().name != previous { println!("Frame {frame}: {}", anim.state().name); }
                        for e in events { println!("Frame {frame}: event {} in {}", e.name, e.state); }
                    }
                }
                ["expect", name] if anim.state().name == *name => (),
                ["expect", name] => return Err(format!("{}: expected {name}, reached {}", location(), anim.state().name)),
                _ => return Err(format!("{}: unknown command: {line}\n{USAGE}", location())),
            }
        }
    }
    let (picture, clip, index) = look.sample(&anim.state().clip, anim.sample_time(|c| look.duration(c)), anim.blend.as_ref());
    if let Some(file) = shot {
        let path = Path::new(&file);
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|e| format!("{file}: {e}"))?;
        }
        image::save_buffer_with_format(path, &picture.rgba, picture.w as u32, picture.h as u32,
            image::ColorType::Rgba8, image::ImageFormat::Png).map_err(|e| format!("{file}: {e}"))?;
    }
    println!("PASS at frame {frame}: state {}, clip {clip}, sprite {index}", anim.state().name);
    Ok(())
}

fn main() {
    if let Err(e) = run() { eprintln!("{e}"); std::process::exit(3); }
}
