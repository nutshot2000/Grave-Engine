//! Looks: how a character is drawn. A look is a folder of animation frames
//! (pictures at any resolution, e.g. rendered from a Blender character) plus
//! a small `character.txt`:
//!
//! ```text
//! frame = 256          # each picture is this many pixels square (or `frame = 256 320`)
//! figure = 192         # how tall the character stands in a picture (pixels)
//! feet = 234.7         # the row its feet are on
//! faces = right        # which way it looks in the pictures
//! idle = 16 frames, 8 fps, loop      # idle_00.png ... idle_15.png
//! walk = 16 frames, 16 fps, loop
//! jump = 6 frames, 12 fps            # plays once, then holds the last frame
//! fall = 8 frames, 10 fps, loop
//! swim = 16 frames, 10 fps, loop
//! die = 14 frames, 12 fps
//! ```
//!
//! The pictures are drawn sharp, on top of the cell world. Each frame's
//! outline is also turned into cells, so the character's body in the world
//! (what sand piles against, what fire heats) matches the picture: a raised
//! arm really is up there.

use crate::import::Picture;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The character's body is worked out on this many cells across (2 m), and
/// `physics::actor::H` cells high.
pub const MASK_W: usize = 20;

pub struct Frame {
    pub w: usize,
    pub h: usize,
    /// RGBA pixels, row by row.
    pub rgba: Vec<u8>,
    /// Which body cells this frame covers (`MASK_W` x `H`, row by row),
    /// for a character facing the way the pictures do.
    pub mask: Vec<bool>,
}

pub struct Anim {
    pub frames: Vec<Frame>,
    pub fps: f32,
    pub looping: bool,
}

pub struct Look {
    /// Folder name (what scenes call it).
    pub name: String,
    pub dir: PathBuf,
    /// Character height and feet row in the pictures, in pixels.
    pub figure: f32,
    pub feet: f32,
    pub faces_right: bool,
    pub anims: BTreeMap<String, Anim>,
}

impl std::fmt::Debug for Look {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Look({})", self.name)
    }
}

impl PartialEq for Look {
    fn eq(&self, other: &Self) -> bool {
        self.dir == other.dir
    }
}

impl Look {
    /// Load a look from its folder.
    pub fn load(dir: &Path, body_h: usize) -> Result<Look, String> {
        let file = dir.join("character.txt");
        let text = std::fs::read_to_string(&file).map_err(|e| format!("can't read {}: {e}", file.display()))?;
        let mut figure = None;
        let mut feet = None;
        let mut faces_right = true;
        let mut wanted: Vec<(String, usize, f32, bool)> = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.split(" #").next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("{} line {}: expected `key = value`", file.display(), n + 1));
            };
            let (key, value) = (key.trim().to_lowercase(), value.trim());
            let num = |v: &str| v.split_whitespace().next().and_then(|w| w.parse::<f32>().ok());
            match key.as_str() {
                "frame" => {}
                "figure" => figure = num(value),
                "feet" => feet = num(value),
                "faces" => faces_right = !value.eq_ignore_ascii_case("left"),
                anim => {
                    // `N frames, F fps, loop`
                    let mut count = None;
                    let mut fps = 10.0;
                    let mut looping = false;
                    for part in value.split(',') {
                        let part = part.trim().to_lowercase();
                        if part == "loop" {
                            looping = true;
                        } else if part.ends_with("fps") {
                            fps = num(&part).unwrap_or(10.0);
                        } else if part.ends_with("frames") || part.ends_with("frame") {
                            count = num(&part).map(|c| c as usize);
                        }
                    }
                    let Some(count) = count.filter(|c| *c > 0) else {
                        return Err(format!("{} line {}: use `{anim} = 8 frames, 10 fps, loop`", file.display(), n + 1));
                    };
                    wanted.push((anim.to_string(), count, fps, looping));
                }
            }
        }
        let figure = figure.ok_or(format!("{}: missing `figure = ...`", file.display()))?;
        let feet = feet.ok_or(format!("{}: missing `feet = ...`", file.display()))?;
        let mut anims = BTreeMap::new();
        for (name, count, fps, looping) in wanted {
            let mut frames = Vec::new();
            for i in 0..count {
                let path = dir.join(format!("{name}_{i:02}.png"));
                let pic = Picture::open(&path)?;
                let mask = outline(&pic, figure, feet, body_h);
                frames.push(Frame { w: pic.w, h: pic.h, rgba: pic.rgba, mask });
            }
            anims.insert(name, Anim { frames, fps, looping });
        }
        if anims.is_empty() {
            return Err(format!("{}: no animations listed", file.display()));
        }
        let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        Ok(Look { name, dir: dir.to_path_buf(), figure, feet, faces_right, anims })
    }

    /// The frame of `anim` (or idle, if it hasn't got one) `t` seconds in,
    /// and its number.
    pub fn frame(&self, anim: &str, t: f32) -> (&Frame, usize) {
        let a = self.anims.get(anim).or_else(|| self.anims.get("idle")).or_else(|| self.anims.values().next()).unwrap();
        let n = a.frames.len();
        let mut i = (t.max(0.0) * a.fps) as usize;
        i = if a.looping { i % n } else { i.min(n - 1) };
        (&a.frames[i], i)
    }

    /// How many world cells one picture pixel is, for a body `body_h` cells tall.
    pub fn cells_per_px(&self, body_h: usize) -> f32 {
        body_h as f32 / self.figure
    }
}

/// Which body cells a picture covers: the picture is scaled so the figure
/// is `body_h` cells tall, centred, feet on the bottom row.
fn outline(pic: &Picture, figure: f32, feet: f32, body_h: usize) -> Vec<bool> {
    let px = figure / body_h as f32; // picture pixels per cell
    let mut mask = vec![false; MASK_W * body_h];
    for cy in 0..body_h {
        for cx in 0..MASK_W {
            let x0 = pic.w as f32 / 2.0 + (cx as f32 - MASK_W as f32 / 2.0) * px;
            let y0 = feet - (body_h - cy) as f32 * px;
            let (mut on, mut all) = (0, 0);
            let steps = (px.ceil() as usize).clamp(1, 16);
            for sy in 0..steps {
                for sx in 0..steps {
                    let x = x0 + (sx as f32 + 0.5) * px / steps as f32;
                    let y = y0 + (sy as f32 + 0.5) * px / steps as f32;
                    all += 1;
                    if x >= 0.0 && y >= 0.0 && (x as usize) < pic.w && (y as usize) < pic.h && pic.rgba[((y as usize) * pic.w + x as usize) * 4 + 3] > 128 {
                        on += 1;
                    }
                }
            }
            mask[cy * MASK_W + cx] = on as f32 / all as f32 > 0.35;
        }
    }
    mask
}

/// Loaded looks, shared (loading the frames once is plenty).
static CACHE: Mutex<BTreeMap<PathBuf, Arc<Look>>> = Mutex::new(BTreeMap::new());

/// Find and load a look by name (a folder in `assets/characters/`) or path.
/// `base` is where the scene file is.
pub fn find(name: &str, base: Option<&Path>, body_h: usize) -> Result<Arc<Look>, String> {
    let mut tries: Vec<PathBuf> = Vec::new();
    if let Some(b) = base {
        tries.push(b.join(name));
        tries.push(b.join("../assets/characters").join(name));
        tries.push(b.join("assets/characters").join(name));
    }
    tries.push(PathBuf::from("assets/characters").join(name));
    tries.push(PathBuf::from(name));
    tries.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/characters").join(name));
    let dir = tries
        .into_iter()
        .find(|d| d.join("character.txt").is_file())
        .ok_or(format!("no character called `{name}` (looked for assets/characters/{name}/character.txt)"))?;
    let key = dir.canonicalize().unwrap_or(dir.clone());
    if let Some(l) = CACHE.lock().unwrap().get(&key) {
        return Ok(l.clone());
    }
    let look = Arc::new(Look::load(&dir, body_h)?);
    CACHE.lock().unwrap().insert(key, look.clone());
    Ok(look)
}
