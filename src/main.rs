use grave_engine::headless;
use std::path::PathBuf;

mod app;

const HELP: &str = "\
Grave Engine - a 2D material sandbox

  grave-engine                      open the sandbox window
  grave-engine scenes/x.txt         open the window with a scene loaded

Headless (no window), for agents and scripts:
  grave-engine --run scenes/x.txt --frames 600 --shot shots/out.png
      --frames N      how many frames to simulate (default 600)
      --shot PATH     where to save the final screenshot (PNG)
      --every N       also save a screenshot every N frames
      --scale S       make screenshots S times bigger (default 2)
      --heat          screenshot the heat view instead
      --throttle T    hold the drive controls (1 = full ahead, -1 = reverse)
      --walk W        hold the player's walk controls (1 = right, -1 = left)
      --jump          hold the player's jump
      --materials DIR use a different materials folder (default materials)
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut run = None;
    let mut scene = None;
    let mut frames = 600;
    let mut shot = None;
    let mut every = None;
    let mut scale = 2;
    let mut heat = false;
    let mut throttle = 0.0f32;
    let mut walk = 0.0f32;
    let mut jump = false;
    let mut materials = PathBuf::from("materials");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |name: &str| -> String {
            it.next().cloned().unwrap_or_else(|| {
                eprintln!("{name} needs a value\n\n{HELP}");
                std::process::exit(2);
            })
        };
        match a.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return;
            }
            "--run" => run = Some(PathBuf::from(val("--run"))),
            "--frames" => frames = val("--frames").parse().unwrap_or(600),
            "--shot" => shot = Some(PathBuf::from(val("--shot"))),
            "--every" => every = val("--every").parse().ok(),
            "--scale" => scale = val("--scale").parse().unwrap_or(2),
            "--heat" => heat = true,
            "--throttle" => throttle = val("--throttle").parse().unwrap_or(0.0),
            "--walk" => walk = val("--walk").parse().unwrap_or(0.0),
            "--jump" => jump = true,
            "--materials" => materials = PathBuf::from(val("--materials")),
            other if !other.starts_with('-') => scene = Some(PathBuf::from(other)),
            other => {
                eprintln!("unknown option {other}\n\n{HELP}");
                std::process::exit(2);
            }
        }
    }
    if let Some(scene) = run {
        let code = headless::run(&headless::Options { scene, materials, frames, shot, every, scale, heat, throttle, walk, jump });
        std::process::exit(code);
    }
    if let Err(e) = app::run(materials, scene) {
        eprintln!("couldn't open the window: {e}");
        std::process::exit(1);
    }
}
