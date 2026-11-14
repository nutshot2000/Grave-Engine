use grave_engine::headless;
use std::path::PathBuf;

mod app;
mod workshop;
mod studio;

const HELP: &str = "\
Grave Engine - a 2D material sandbox

  grave-engine                      open the sandbox window
  grave-engine scenes/x.txt         open the window with a scene loaded
  grave-engine --workshop           open the material asset workshop

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
      --player-view N screenshot just N cells across around the player
      --materials DIR use a different materials folder (default materials)
      --no-copy       run the editor from this file (normally it runs from a copy,
                      so the engine can be rebuilt while it's open)

Characters:
  grave-engine --make-character DOWNLOADS/knight [--name knight]
      turn a folder of animated .fbx files (e.g. from Mixamo: Idle.fbx,
      Walking.fbx, Jumping.fbx, Falling Idle.fbx, Treading Water.fbx,
      Dying.fbx) into assets/characters/NAME, using Blender. Then use it
      in a scene with `player x y look NAME`.
";

fn main() {
    // Opening the executable from a desktop launcher must find its content too.
    if !std::path::Path::new("materials").is_dir() {
        if let Ok(exe)=std::env::current_exe() {
            if let Some(root)=exe.ancestors().skip(1).find(|p|p.join("materials").is_dir()&&p.join("scenes").is_dir()) {
                let _=std::env::set_current_dir(root);
            }
        }
    }
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
    let mut player_view = 0usize;
    let mut make_character: Option<PathBuf> = None;
    let mut name: Option<String> = None;
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
            "--workshop" => {},
            "--workshop-file" => { let _ = val("--workshop-file"); },
            "--run" => run = Some(PathBuf::from(val("--run"))),
            "--frames" => frames = val("--frames").parse().unwrap_or(600),
            "--shot" => shot = Some(PathBuf::from(val("--shot"))),
            "--every" => every = val("--every").parse().ok(),
            "--scale" => scale = val("--scale").parse().unwrap_or(2),
            "--heat" => heat = true,
            "--throttle" => throttle = val("--throttle").parse().unwrap_or(0.0),
            "--walk" => walk = val("--walk").parse().unwrap_or(0.0),
            "--jump" => jump = true,
            "--make-character" => make_character = Some(PathBuf::from(val("--make-character"))),
            "--name" => name = Some(val("--name")),
            "--player-view" => player_view = val("--player-view").parse().unwrap_or(0),
            "--materials" => materials = PathBuf::from(val("--materials")),
            "--no-copy" => {}
            other if !other.starts_with('-') => scene = Some(PathBuf::from(other)),
            other => {
                eprintln!("unknown option {other}\n\n{HELP}");
                std::process::exit(2);
            }
        }
    }
    if let Some(input) = make_character {
        let name = name.unwrap_or_else(|| {
            let n = input.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            n.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect()
        });
        let out = PathBuf::from("assets/characters").join(&name);
        println!("Making character `{name}` from {} with Blender (this takes a minute)...", input.display());
        match grave_engine::import::make_character(&input, &out) {
            Ok(report) => {
                println!("{report}");
                println!("Done: {}. Use it with `player x y look {name}`.", out.display());
                return;
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
    if let Some(scene) = run {
        let code = headless::run(&headless::Options { scene, materials, frames, shot, every, scale, heat, throttle, walk, jump, player_view });
        std::process::exit(code);
    }
    // The editor runs from a copy of this program, so the real file is never
    // locked: the engine can be rebuilt while you watch, and the editor
    // restarts itself into the new build (see app.rs, `restart_into_new_build`).
    if std::env::var_os(app::ORIGINAL_EXE).is_none() && !args.iter().any(|a| a == "--no-copy") {
        if let Some(()) = run_from_copy(&args) {
            return;
        }
    }
    if let Err(e) = app::run(materials, scene) {
        eprintln!("couldn't open the window: {e}");
        std::process::exit(1);
    }
}

/// Copy this program next to itself (`editor-copies/`) and start the copy
/// instead, remembering where the real one is. Returns None if that didn't
/// work (then this process just runs the editor itself).
fn run_from_copy(args: &[String]) -> Option<()> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.join("editor-copies");
    std::fs::create_dir_all(&dir).ok()?;
    // Tidy old copies (ones still running are locked and stay).
    if let Ok(rd) = std::fs::read_dir(&dir) {
        // Only old program copies: never anything else that might be there.
        for e in rd.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "exe" || x.is_empty())) {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let ext = exe.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_millis();
    let copy = dir.join(format!("grave-engine-{stamp}{ext}"));
    std::fs::copy(&exe, &copy).ok()?;
    std::process::Command::new(&copy).args(args).env(app::ORIGINAL_EXE, &exe).spawn().ok()?;
    Some(())
}
