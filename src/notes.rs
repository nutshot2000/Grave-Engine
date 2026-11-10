//! Notes: little sticky notes left on things in the editor ("make this crate
//! iron", "move this a bit to the right"), for whoever works on the scene
//! next: you later, a friend, or an AI agent.
//!
//! Notes live in a file next to the scene: `player.txt` -> `player.notes.md`,
//! written the moment a note is added or ticked. It's plain Markdown, so it
//! reads well and agents can parse it:
//!
//! ```text
//! # Notes for player.txt
//!
//! - [ ] at 392,256 | on: object #4: wood, 14x12 cells, centre 399,262 | picture: player.notes/note-1.png | make this crate iron
//! - [x] at 100,40 | on: stone (scenery) | move this a bit to the right
//! ```
//!
//! Each note says exactly what it's on: an object's materials, size and
//! position (and template, if it's a copy of one), or the scenery's
//! material, plus a close-up picture with the thing outlined.

use crate::material::{AIR, Materials};
use crate::sim::Sim;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    /// Where the note is pinned (cells).
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub done: bool,
    /// What it's on, in words.
    pub about: String,
    /// A close-up picture of the spot (relative to the notes file), if saved.
    pub picture: Option<String>,
    /// Pinned to an object: (physics run, object id, offset from its centre),
    /// so it follows the object around. Not saved: when a scene is loaded,
    /// notes stick to whatever object is under them.
    pub object: Option<(u64, u64, f32, f32)>,
    /// The physics run this note last looked for its object in (0 = never).
    pub run: u64,
}

impl Note {
    pub fn new(x: f32, y: f32, text: &str) -> Self {
        Note { x, y, text: text.trim().to_string(), done: false, about: String::new(), picture: None, object: None, run: 0 }
    }
}

/// The notes file for a scene file.
pub fn path_for(scene: &Path) -> PathBuf {
    scene.with_extension("notes.md")
}

/// The folder for a scene's note pictures (next to its notes file).
pub fn pictures_dir(scene: &Path) -> PathBuf {
    scene.with_extension("notes")
}

/// Read notes (a missing file is no notes).
pub fn load(path: &Path) -> Vec<Note> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    parse(&text)
}

pub fn parse(text: &str) -> Vec<Note> {
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        let done = if l.starts_with("- [ ]") {
            false
        } else if l.starts_with("- [x]") || l.starts_with("- [X]") {
            true
        } else {
            continue;
        };
        let fields: Vec<&str> = l[5..].split(" | ").map(str::trim).collect();
        let Some(at) = fields.first().and_then(|f| f.strip_prefix("at ")) else { continue };
        let Some((x, y)) = at.split_once(',').and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?))) else { continue };
        let mut note = Note::new(x, y, "");
        note.done = done;
        let mut rest = Vec::new();
        for f in &fields[1..] {
            if let Some(a) = f.strip_prefix("on:") {
                note.about = a.trim().to_string();
            } else if let Some(p) = f.strip_prefix("picture:") {
                note.picture = Some(p.trim().to_string());
            } else {
                rest.push(*f);
            }
        }
        note.text = rest.join(" | ");
        out.push(note);
    }
    out
}

pub fn format(scene_name: &str, notes: &[Note]) -> String {
    let mut s = format!(
        "# Notes for {scene_name}\n\nLeft in the Grave Engine editor (right-click > Leave a note). \
         Each says where it is, what it's on, and has a close-up picture. Tick [x] when done.\n\n"
    );
    for n in notes {
        s += &format!("- [{}] at {:.0},{:.0}", if n.done { "x" } else { " " }, n.x, n.y);
        if !n.about.is_empty() {
            s += &format!(" | on: {}", n.about);
        }
        if let Some(p) = &n.picture {
            s += &format!(" | picture: {p}");
        }
        s += &format!(" | {}\n", n.text.replace('\n', " "));
    }
    s
}

pub fn save(path: &Path, scene_name: &str, notes: &[Note]) -> std::io::Result<()> {
    if notes.is_empty() && !path.exists() {
        return Ok(());
    }
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(path, format(scene_name, notes))
}

/// What's at a spot, in words, and the object there (if any).
pub fn describe(sim: &Sim, mats: &Materials, x: f32, y: f32) -> (String, Option<u64>) {
    let w = &sim.world;
    let (cx, cy) = (x.floor() as i32, y.floor() as i32);
    if !w.in_bounds(cx, cy) {
        return ("outside the world".into(), None);
    }
    if let Some(id) = sim.physics.id_at(w, cx as usize, cy as usize) {
        if let Some(b) = sim.physics.spec_of(w, id) {
            // Materials, most first.
            let mut count: std::collections::BTreeMap<u16, usize> = Default::default();
            for c in b.cells.iter().flatten() {
                *count.entry(c.mat).or_default() += 1;
            }
            let mut by: Vec<(u16, usize)> = count.into_iter().collect();
            by.sort_by(|a, b| b.1.cmp(&a.1));
            let mats_text: Vec<String> = by.iter().take(3).map(|(m, n)| format!("{n} {}", mats.get(*m).name)).collect();
            let index = sim.physics.ids().iter().position(|&i| i == id).map(|p| p + 1).unwrap_or(0);
            let template = sim.instances.get(&id).map(|i| format!(", a copy of template `{}`", i.template)).unwrap_or_default();
            let about = format!(
                "object #{index} ({} cells: {}), {}x{} cells, centre {:.0},{:.0}, turned {:.0}°{template}",
                b.cell_count(),
                mats_text.join(", "),
                b.w,
                b.h,
                b.x,
                b.y,
                b.angle.to_degrees()
            );
            return (about, Some(id));
        }
    }
    let m = w.mat[w.idx(cx as usize, cy as usize)];
    if m == AIR {
        ("empty space".into(), None)
    } else {
        (format!("{} (scenery)", mats.get(m).name), None)
    }
}

/// Keep pinned notes on their objects. After the world was replaced (undo,
/// reset, rewind, load), stick each note to whatever object is under it.
pub fn follow(sim: &Sim, notes: &mut [Note]) {
    let run = sim.physics.run_id();
    for n in notes.iter_mut() {
        if n.run != run {
            // A new world: stick to the object under the note, if any (a
            // note on scenery stays on scenery).
            n.run = run;
            let (cx, cy) = (n.x.floor() as i32, n.y.floor() as i32);
            let hit = sim.world.in_bounds(cx, cy).then(|| sim.physics.id_at(&sim.world, cx as usize, cy as usize)).flatten();
            n.object = hit.and_then(|id| sim.physics.spec_of(&sim.world, id).map(|b| (run, id, n.x - b.x, n.y - b.y)));
        } else if let Some((_, id, dx, dy)) = n.object {
            match sim.physics.spec_of(&sim.world, id) {
                Some(b) => {
                    n.x = b.x + dx;
                    n.y = b.y + dy;
                }
                None => n.object = None, // the object's gone (broke, burned): the note stays put
            }
        }
    }
}
