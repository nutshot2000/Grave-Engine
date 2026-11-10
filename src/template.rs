//! Templates: reusable objects saved as text files in `templates/`.
//!
//! A template file is a tiny scene whose first object is the template's
//! shape, so anything a scene can describe works (`body` blocks with a
//! legend, `body rect ...`, or `object crate.png ...`):
//!
//! ```text
//! # Template: crate
//! legend W wood
//! legend I iron
//! body 0 0 0
//! IIII
//! IWWI
//! IIII
//! end
//! ```
//!
//! Placed copies remember which template they came from plus their own
//! changes, in order ("made of iron", "flipped", "twice as big"). Edit the
//! template and every copy updates, keeping its own changes.

use crate::edit::{self, Transform};
use crate::material::{MatId, Materials};
use crate::physics::BodySpec;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// One change a copy has on top of its template.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    /// Everything turned into this material (by name, so files stay readable).
    Material(String),
    Transform(Transform),
}

impl Change {
    /// Plain words, for the inspector.
    pub fn describe(&self) -> String {
        match self {
            Change::Material(m) => format!("made of {}", m.replace('_', " ")),
            Change::Transform(Transform::FlipH) => "flipped left-right".into(),
            Change::Transform(Transform::FlipV) => "flipped upside down".into(),
            Change::Transform(Transform::Turn) => "quarter turn".into(),
            Change::Transform(Transform::Scale(f)) => format!("size ×{f}"),
        }
    }

    /// The words used in scene files.
    pub fn to_words(&self) -> String {
        match self {
            Change::Material(m) => format!("material {m}"),
            Change::Transform(Transform::FlipH) => "flip".into(),
            Change::Transform(Transform::FlipV) => "flipv".into(),
            Change::Transform(Transform::Turn) => "turn".into(),
            Change::Transform(Transform::Scale(f)) => format!("scale {f}"),
        }
    }
}

/// Read changes from scene words like `material iron flip scale 2`.
/// Returns the changes and any words it didn't understand.
pub fn parse_changes(words: &[&str]) -> (Vec<Change>, Vec<String>) {
    let mut out = Vec::new();
    let mut bad = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let w = words[i].to_lowercase();
        match w.as_str() {
            "material" | "as" if i + 1 < words.len() => {
                out.push(Change::Material(words[i + 1].to_lowercase()));
                i += 1;
            }
            "flip" | "fliph" => out.push(Change::Transform(Transform::FlipH)),
            "flipv" => out.push(Change::Transform(Transform::FlipV)),
            "turn" => out.push(Change::Transform(Transform::Turn)),
            "scale" if i + 1 < words.len() => {
                match words[i + 1].parse::<f32>() {
                    Ok(f) => out.push(Change::Transform(Transform::Scale(f))),
                    Err(_) => bad.push(format!("scale {}", words[i + 1])),
                }
                i += 1;
            }
            _ => bad.push(w),
        }
        i += 1;
    }
    (out, bad)
}

/// A placed copy of a template.
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub template: String,
    pub changes: Vec<Change>,
}

/// A loaded template.
#[derive(Clone, Debug)]
pub struct Template {
    pub name: String,
    /// Its shape (position and angle are ignored).
    pub shape: BodySpec,
    pub file: PathBuf,
}

/// Build a copy's shape: the template's cells with the copy's changes applied.
pub fn build(shape: &BodySpec, changes: &[Change], mats: &Materials) -> BodySpec {
    let mut s = shape.clone();
    for c in changes {
        match c {
            Change::Material(name) => {
                if let Some(id) = mats.id(name) {
                    edit::recolor(&mut s.cells, mats, id);
                }
            }
            Change::Transform(t) => edit::transform_body(&mut s, *t),
        }
    }
    s
}

/// Load one template file. Its name is the file name.
pub fn load_file(path: &Path, mats: &Materials) -> Result<Template, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path.file_stem().unwrap_or_default().to_string_lossy().to_lowercase();
    let loaded = crate::scene::load_part(&text, mats, path.parent(), false);
    if let Some(p) = loaded.problems.first() {
        return Err(format!("template {name}: {p}"));
    }
    let shape = loaded.bodies.into_iter().next().ok_or(format!("template {name}: it has no object in it"))?;
    Ok(Template { name, shape, file: path.to_path_buf() })
}

/// Where to look for a template: next to the scene, in `templates/` near it,
/// or in `templates/` in the current folder.
pub fn find(name: &str, base: Option<&Path>) -> Option<PathBuf> {
    let file = format!("{}.txt", name.to_lowercase());
    let mut places = Vec::new();
    if let Some(b) = base {
        places.push(b.join("templates").join(&file));
        places.push(b.join("..").join("templates").join(&file));
    }
    places.push(Path::new("templates").join(&file));
    places.into_iter().find(|p| p.exists())
}

/// Write a shape as a template file. Returns its path.
pub fn save(dir: &Path, name: &str, shape: &BodySpec, mats: &Materials) -> Result<PathBuf, String> {
    let name = clean_name(name).ok_or("give the template a name (letters, numbers, - and _)")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{name}.txt"));
    let pool: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789%&*+=?@$~^!:;<>/|()[]{}-_,".chars().collect();
    let mut chars: HashMap<MatId, char> = HashMap::new();
    let mut used = vec!['.'];
    let mut out = format!("# Template: {name}. Edit freely: every placed copy updates.\n");
    for c in shape.cells.iter().flatten() {
        if chars.contains_key(&c.mat) {
            continue;
        }
        let m = &mats.get(c.mat).name;
        let first = m.chars().next().unwrap_or('X');
        let pick = [first.to_ascii_uppercase(), first.to_ascii_lowercase()]
            .into_iter()
            .chain(pool.iter().copied())
            .find(|ch| !used.contains(ch))
            .unwrap_or('?');
        used.push(pick);
        chars.insert(c.mat, pick);
        out.push_str(&format!("legend {pick} {m}\n"));
    }
    out.push_str("body 0 0 0\n");
    for y in 0..shape.h {
        let row: String = (0..shape.w).map(|x| shape.cells[y * shape.w + x].map(|c| chars[&c.mat]).unwrap_or('.')).collect();
        out.push_str(&row);
        out.push('\n');
    }
    out.push_str("end\n");
    std::fs::write(&path, out).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Make a name safe for a file name.
pub fn clean_name(name: &str) -> Option<String> {
    let n: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c == ' ' { '_' } else { c })
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    (!n.is_empty()).then_some(n)
}

/// All templates in a folder, with live reload.
#[derive(Default)]
pub struct Templates {
    pub list: Vec<Template>,
    pub problems: Vec<String>,
    dir: PathBuf,
    stamps: Vec<(PathBuf, Option<SystemTime>)>,
}

fn stamps(dir: &Path) -> Vec<(PathBuf, Option<SystemTime>)> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
                .map(|e| (e.path(), e.metadata().and_then(|m| m.modified()).ok()))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

impl Templates {
    pub fn load_dir(dir: impl AsRef<Path>, mats: &Materials) -> Self {
        let mut t = Templates { dir: dir.as_ref().to_path_buf(), ..Default::default() };
        t.reload(mats);
        t
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn reload(&mut self, mats: &Materials) {
        self.list.clear();
        self.problems.clear();
        self.stamps = stamps(&self.dir);
        for (path, _) in self.stamps.clone() {
            match load_file(&path, mats) {
                Ok(t) => self.list.push(t),
                Err(e) => self.problems.push(e),
            }
        }
        self.list.sort_by(|a, b| a.name.cmp(&b.name));
    }

    pub fn changed_on_disk(&self) -> bool {
        stamps(&self.dir) != self.stamps
    }

    pub fn get(&self, name: &str) -> Option<&Template> {
        self.list.iter().find(|t| t.name == name)
    }
}

/// Does an object still look like its template (with its changes)? If it's
/// been burned, chipped or painted on, it has gone its own way.
pub fn matches(spec: &BodySpec, shape: &BodySpec) -> bool {
    spec.w == shape.w
        && spec.h == shape.h
        && spec.cells.iter().zip(&shape.cells).all(|(a, b)| a.map(|c| c.mat) == b.map(|c| c.mat))
}

/// Rebuild every copy from its template (after a template changed), keeping
/// each copy's position, angle, motion and own changes. Copies whose
/// template is gone are left as they are. Returns how many were updated.
pub fn refresh(sim: &mut crate::sim::Sim, templates: &Templates, mats: &Materials) -> usize {
    let copies: Vec<(u64, Instance)> = sim.instances.iter().map(|(id, i)| (*id, i.clone())).collect();
    let mut n = 0;
    for (id, inst) in copies {
        let Some(t) = templates.get(&inst.template) else { continue };
        let built = build(&t.shape, &inst.changes, mats);
        if sim.physics.edit(&mut sim.world, mats, id, |s| {
            s.w = built.w;
            s.h = built.h;
            s.cells = built.cells.clone();
        }) {
            n += 1;
        }
    }
    n
}

/// Place a copy of a template centred at (x, y). Returns its object id.
pub fn place(sim: &mut crate::sim::Sim, t: &Template, x: f32, y: f32, mats: &Materials) -> Option<u64> {
    let mut spec = t.shape.clone();
    spec.x = x;
    spec.y = y;
    spec.angle = 0.0;
    spec.vx = 0.0;
    spec.vy = 0.0;
    spec.spin = 0.0;
    let id = sim.physics.add(&mut sim.world, mats, spec)?;
    sim.instances.insert(id, Instance { template: t.name.clone(), changes: Vec::new() });
    Some(id)
}
