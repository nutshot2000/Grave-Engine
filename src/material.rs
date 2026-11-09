//! Materials: loaded from plain-text files in `materials/`, one file per material.
//!
//! A material file is a list of `key = value` lines. Lines starting with `#`,
//! and anything after ` # ` on a line, are comments. Temperatures are in °C.
//! See `materials/README.md` for every key and what it does.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub type MatId = u16;
/// Air is always material 0. If there's no `air.txt`, a built-in air is used.
pub const AIR: MatId = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Stays where it is (stone, wood, iron, ice).
    Solid,
    /// Falls and piles up (sand, ash).
    Powder,
    /// Falls and spreads sideways (water, oil, lava).
    Liquid,
    /// Rises and drifts (steam, smoke, fire, air).
    Gas,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            State::Solid => "solid",
            State::Powder => "powder",
            State::Liquid => "liquid",
            State::Gas => "gas",
        })
    }
}

/// "When I touch `with`, I become `me_into` and it becomes `other_into`."
#[derive(Clone, Debug)]
pub struct Reaction {
    pub with: MatId,
    pub me_into: MatId,
    pub other_into: MatId,
    /// Chance per frame (0..1) while touching.
    pub chance: f32,
}

/// A temperature threshold and the material it turns into.
#[derive(Clone, Copy, Debug)]
pub struct Change {
    pub at: f32,
    pub into: MatId,
}

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub about: String,
    pub state: State,
    /// kg per cubic metre. Heavier things sink below lighter ones.
    pub density: f32,
    pub color: [u8; 3],
    /// How much each cell's colour varies (0 = flat, 0.3 = very speckled).
    pub color_variation: f32,
    /// How much it lights up its surroundings (0..1).
    pub glow: f32,
    /// Temperature a freshly painted cell starts at.
    pub temperature: f32,
    /// How easily heat passes through it (0..1).
    pub conducts_heat: f32,
    /// How much heat it takes to warm it up (1 = normal, 4 = water).
    pub heat_capacity: f32,
    /// How quickly it drifts back to room temperature by itself (0..1 per frame).
    pub cools_off: f32,
    /// If set, the cell is held at this temperature (a heat source, like fire).
    pub stays_at: Option<f32>,
    /// Burning: catches fire at this temperature when it touches something that feeds fire.
    pub catches_fire_at: Option<f32>,
    /// How many frames it keeps burning.
    pub burns_for: u16,
    /// How hot it burns.
    pub burn_temperature: f32,
    /// What's left when it finishes burning.
    pub burns_into: MatId,
    /// Chance (0..1) that a burned-out cell leaves `burns_into` behind; otherwise it
    /// burns away completely (wood leaves a little ash, not a pile the size of the wood).
    pub burns_into_chance: f32,
    /// What it puffs out while burning (into neighbouring air).
    pub gives_off: Option<MatId>,
    /// Chance per frame of puffing out `gives_off`.
    pub gives_off_chance: f32,
    /// Does it feed fire? (Air does: things only burn when touching it.)
    pub feeds_fire: bool,
    /// Does it put out fires it touches? (Water does.)
    pub puts_out_fire: bool,
    /// Falling objects crush it to nothing instead of landing on it (ash).
    pub crushable: bool,
    pub melts: Option<Change>,
    pub boils: Option<Change>,
    pub freezes: Option<Change>,
    pub condenses: Option<Change>,
    /// If set, the cell only lasts about this many frames...
    pub lifetime: Option<u16>,
    /// ...and then turns into this.
    pub decays_into: MatId,
    /// Chance (0..1) that it turns into `decays_into` when it runs out; otherwise
    /// it just becomes air (a flame only sometimes leaves smoke).
    pub decays_into_chance: f32,
    /// Liquids/gases: how far it can spread sideways per frame.
    pub flows: u8,
    /// Solids: how many cells it can stick out from a support before it snaps.
    pub strength: f32,
    /// How hard a hit (or how much weight) it takes to crack, in kilonewtons.
    /// `None` = it never cracks.
    pub toughness: Option<f32>,
    /// What cracked cells turn into (stone -> gravel). `None` = it splinters
    /// into small loose pieces instead.
    pub breaks_into: Option<MatId>,
    /// How grippy it is for objects sliding on it (0 = ice-slick, 1 = rubbery).
    pub friction: f32,
    /// How much objects bounce off it (0 = thud, 1 = superball).
    pub bounciness: f32,
    /// Explosives: blast radius in cells when it goes off (0 = not explosive).
    pub explosion_size: f32,
    /// Explosives: how hard the blast hits at its centre, in kilonewtons.
    pub explosion_power: f32,
    pub reactions: Vec<Reaction>,
    /// Where it was loaded from (None for built-ins).
    pub file: Option<PathBuf>,
}

impl Material {
    fn blank(name: &str) -> Self {
        Material {
            name: name.to_string(),
            about: String::new(),
            state: State::Solid,
            density: 1000.0,
            color: [255, 0, 255],
            color_variation: 0.06,
            glow: 0.0,
            temperature: 20.0,
            conducts_heat: 0.2,
            heat_capacity: 1.0,
            cools_off: 0.0,
            stays_at: None,
            catches_fire_at: None,
            burns_for: 100,
            burn_temperature: 800.0,
            burns_into: AIR,
            burns_into_chance: 1.0,
            gives_off: None,
            gives_off_chance: 0.1,
            feeds_fire: false,
            puts_out_fire: false,
            crushable: false,
            melts: None,
            boils: None,
            freezes: None,
            condenses: None,
            lifetime: None,
            decays_into: AIR,
            decays_into_chance: 1.0,
            flows: 4,
            strength: 1.0,
            toughness: None,
            breaks_into: None,
            friction: 0.6,
            bounciness: 0.05,
            explosion_size: 0.0,
            explosion_power: 0.0,
            reactions: Vec::new(),
            file: None,
        }
    }

    fn builtin_air() -> Self {
        let mut m = Material::blank("air");
        m.about = "Empty space. Feeds fire.".into();
        m.state = State::Gas;
        m.density = 1.2;
        m.color = [18, 17, 24];
        m.color_variation = 0.0;
        m.conducts_heat = 0.05;
        m.heat_capacity = 1.0;
        m.cools_off = 0.01;
        m.feeds_fire = true;
        m
    }
}

/// All loaded materials. Material ids stay the same across reloads (by name),
/// so a running world keeps working when files change.
#[derive(Clone, Debug)]
pub struct Materials {
    pub list: Vec<Material>,
    pub by_name: HashMap<String, MatId>,
    /// Problems found while loading (bad values, unknown keys, missing names).
    pub problems: Vec<String>,
    pub dir: Option<PathBuf>,
    stamp: Vec<(PathBuf, Option<SystemTime>)>,
}

/// Raw text of one file before names are turned into ids.
struct Raw {
    name: String,
    file: Option<PathBuf>,
    pairs: Vec<(usize, String, String)>,
}

impl Materials {
    /// Load every `*.txt` file in `dir`.
    pub fn load_dir(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref().to_path_buf();
        let mut m = Materials {
            list: Vec::new(),
            by_name: HashMap::new(),
            problems: Vec::new(),
            dir: Some(dir.clone()),
            stamp: Vec::new(),
        };
        m.reload();
        m
    }

    /// Build from in-memory texts: `(file name, contents)`. Handy for tests.
    pub fn from_texts(texts: &[(&str, &str)]) -> Self {
        let mut m = Materials {
            list: Vec::new(),
            by_name: HashMap::new(),
            problems: Vec::new(),
            dir: None,
            stamp: Vec::new(),
        };
        let raws = texts
            .iter()
            .map(|(name, text)| parse_raw(name, None, text, &mut m.problems))
            .collect();
        m.build(raws);
        m
    }

    pub fn get(&self, id: MatId) -> &Material {
        &self.list[id as usize]
    }

    pub fn id(&self, name: &str) -> Option<MatId> {
        self.by_name.get(&name.trim().to_lowercase()).copied()
    }

    /// Re-read all files from disk. Existing names keep their ids.
    pub fn reload(&mut self) {
        let Some(dir) = self.dir.clone() else { return };
        self.problems.clear();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|x| x == "txt"))
                    .collect()
            })
            .unwrap_or_default();
        if files.is_empty() {
            self.problems
                .push(format!("No material files found in {}", dir.display()));
        }
        files.sort();
        let mut raws = Vec::new();
        for f in &files {
            match std::fs::read_to_string(f) {
                Ok(text) => {
                    let stem = f.file_stem().unwrap_or_default().to_string_lossy();
                    raws.push(parse_raw(&stem, Some(f.clone()), &text, &mut self.problems));
                }
                Err(e) => self.problems.push(format!("{}: can't read ({e})", f.display())),
            }
        }
        self.stamp = stamps(&dir);
        self.build(raws);
    }

    /// True if any file in the materials folder was added, removed or changed
    /// since the last load.
    pub fn changed_on_disk(&self) -> bool {
        match &self.dir {
            Some(dir) => stamps(dir) != self.stamp,
            None => false,
        }
    }

    fn build(&mut self, raws: Vec<Raw>) {
        // Assign ids: keep old ids for names we already know, air is always 0.
        let mut by_name: HashMap<String, MatId> = HashMap::new();
        let mut names: Vec<String> = Vec::new();
        let old: Vec<String> = self.list.iter().map(|m| m.name.clone()).collect();
        if old.is_empty() {
            names.push("air".into());
        } else {
            names = old;
        }
        for r in &raws {
            if !names.contains(&r.name) {
                names.push(r.name.clone());
            }
        }
        for (i, n) in names.iter().enumerate() {
            by_name.insert(n.clone(), i as MatId);
        }

        let mut list: Vec<Material> = names
            .iter()
            .map(|n| {
                if n == "air" {
                    Material::builtin_air()
                } else {
                    // A material that was deleted from disk: keep a pink placeholder.
                    let mut m = Material::blank(n);
                    m.about = "This material's file is missing.".into();
                    m
                }
            })
            .collect();

        for raw in raws {
            let id = by_name[&raw.name] as usize;
            let mut mat = if raw.name == "air" {
                Material::builtin_air()
            } else {
                Material::blank(&raw.name)
            };
            mat.file = raw.file.clone();
            let label = raw
                .file
                .as_ref()
                .map(|f| f.display().to_string())
                .unwrap_or_else(|| raw.name.clone());
            let mut burn_set = false;
            for (line, key, value) in &raw.pairs {
                if let Err(e) = apply(&mut mat, key, value, &by_name, &mut burn_set) {
                    self.problems.push(format!("{label} line {line}: {e}"));
                }
            }
            // Sensible default: burning things puff out fire.
            if mat.catches_fire_at.is_some() && !burn_set {
                mat.gives_off = by_name.get("fire").copied();
            }
            list[id] = mat;
        }
        self.list = list;
        self.by_name = by_name;
    }
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

/// Strip a comment: a line starting with `#`, or ` # ...` after a value.
/// (A colour like `#8b5a2b` is not a comment because no space follows the `#`.)
pub fn strip_comment(line: &str) -> &str {
    let t = line.trim_start();
    if t.starts_with("# ") || t == "#" || t.starts_with("##") {
        return "";
    }
    let bytes = line.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'#'
            && (i == 0 || bytes[i - 1].is_ascii_whitespace())
            && (i + 1 == bytes.len() || bytes[i + 1].is_ascii_whitespace())
        {
            return &line[..i];
        }
    }
    line
}

fn parse_raw(file_stem: &str, file: Option<PathBuf>, text: &str, problems: &mut Vec<String>) -> Raw {
    let mut name = file_stem.trim().to_lowercase();
    let mut pairs = Vec::new();
    let label = file
        .as_ref()
        .map(|f| f.display().to_string())
        .unwrap_or_else(|| file_stem.to_string());
    for (n, line) in text.lines().enumerate() {
        let line = strip_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            problems.push(format!("{label} line {}: expected `key = value`", n + 1));
            continue;
        };
        let k = k.trim().to_lowercase().replace([' ', '-'], "_");
        let v = v.trim().to_string();
        if k == "name" {
            name = v.to_lowercase();
        } else {
            pairs.push((n + 1, k, v));
        }
    }
    Raw { name, file, pairs }
}

fn num(v: &str) -> Result<f32, String> {
    let v = v.trim().trim_end_matches("°c").trim_end_matches('c').trim();
    v.parse::<f32>()
        .map_err(|_| format!("`{v}` is not a number"))
}

fn is_none(v: &str) -> bool {
    matches!(v.trim().to_lowercase().as_str(), "none" | "never" | "no" | "-" | "")
}

fn opt_num(v: &str) -> Result<Option<f32>, String> {
    if is_none(v) { Ok(None) } else { num(v).map(Some) }
}

fn yes_no(v: &str) -> Result<bool, String> {
    match v.trim().to_lowercase().as_str() {
        "yes" | "true" | "on" | "1" => Ok(true),
        "no" | "false" | "off" | "0" => Ok(false),
        _ => Err(format!("`{v}` should be yes or no")),
    }
}

fn name_to_id(v: &str, by_name: &HashMap<String, MatId>) -> Result<MatId, String> {
    let n = v.trim().to_lowercase();
    if is_none(&n) || n == "nothing" {
        return Ok(AIR);
    }
    by_name
        .get(&n)
        .copied()
        .ok_or_else(|| format!("there is no material called `{n}`"))
}

pub fn parse_color(v: &str) -> Result<[u8; 3], String> {
    let v = v.trim();
    if let Some(hex) = v.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(n) = u32::from_str_radix(hex, 16) {
                return Ok([(n >> 16) as u8, (n >> 8) as u8, n as u8]);
            }
        }
        return Err(format!("`{v}` isn't a colour like #ff8800"));
    }
    let parts: Vec<&str> = v.split([' ', ',']).filter(|s| !s.is_empty()).collect();
    if parts.len() == 3 {
        let mut c = [0u8; 3];
        for (i, p) in parts.iter().enumerate() {
            c[i] = p.parse().map_err(|_| format!("`{v}` isn't a colour like #ff8800 or 255 136 0"))?;
        }
        return Ok(c);
    }
    Err(format!("`{v}` isn't a colour like #ff8800 or 255 136 0"))
}

/// `water -> stone + steam` or `water -> stone + steam, chance 0.3`
fn parse_reaction(v: &str, by_name: &HashMap<String, MatId>) -> Result<Reaction, String> {
    let (body, chance) = match v.split_once(',') {
        Some((b, c)) => {
            let c = c.trim().trim_start_matches("chance").trim();
            (b, num(c)?)
        }
        None => (v, 1.0),
    };
    let (with, result) = body
        .split_once("->")
        .ok_or("reactions look like `water -> stone + steam`")?;
    let (me, other) = result
        .split_once('+')
        .ok_or("reactions look like `water -> stone + steam` (what I become + what it becomes)")?;
    Ok(Reaction {
        with: name_to_id(with, by_name)?,
        me_into: name_to_id(me, by_name)?,
        other_into: name_to_id(other, by_name)?,
        chance: chance.clamp(0.0, 1.0),
    })
}

fn apply(
    m: &mut Material,
    key: &str,
    v: &str,
    by_name: &HashMap<String, MatId>,
    gives_off_set: &mut bool,
) -> Result<(), String> {
    match key {
        "about" | "description" => m.about = v.to_string(),
        "state" | "kind" => {
            m.state = match v.to_lowercase().as_str() {
                "solid" => State::Solid,
                "powder" | "sand" | "dust" => State::Powder,
                "liquid" => State::Liquid,
                "gas" => State::Gas,
                _ => return Err(format!("state `{v}` should be solid, powder, liquid or gas")),
            }
        }
        "density" => m.density = num(v)?.max(0.001),
        "color" | "colour" => m.color = parse_color(v)?,
        "color_variation" | "colour_variation" => m.color_variation = num(v)?.clamp(0.0, 1.0),
        "glow" => m.glow = num(v)?.clamp(0.0, 1.0),
        "temperature" | "starts_at" => m.temperature = num(v)?,
        "conducts_heat" => m.conducts_heat = num(v)?.clamp(0.0, 1.0),
        "heat_capacity" => m.heat_capacity = num(v)?.max(0.1),
        "cools_off" => m.cools_off = num(v)?.clamp(0.0, 1.0),
        "stays_at" => m.stays_at = opt_num(v)?,
        "catches_fire_at" => m.catches_fire_at = opt_num(v)?,
        "burns_for" => m.burns_for = num(v)?.clamp(1.0, 60000.0) as u16,
        "burn_temperature" | "burns_at" => m.burn_temperature = num(v)?,
        "burns_into" => m.burns_into = name_to_id(v, by_name)?,
        "burns_into_chance" => m.burns_into_chance = num(v)?.clamp(0.0, 1.0),
        "gives_off" => {
            *gives_off_set = true;
            m.gives_off = if is_none(v) { None } else { Some(name_to_id(v, by_name)?) };
        }
        "gives_off_chance" => m.gives_off_chance = num(v)?.clamp(0.0, 1.0),
        "feeds_fire" => m.feeds_fire = yes_no(v)?,
        "puts_out_fire" => m.puts_out_fire = yes_no(v)?,
        "crushable" => m.crushable = yes_no(v)?,
        "melts_at" => set_at(&mut m.melts, v)?,
        "melts_into" => set_into(&mut m.melts, v, by_name)?,
        "boils_at" => set_at(&mut m.boils, v)?,
        "boils_into" => set_into(&mut m.boils, v, by_name)?,
        "freezes_at" => set_at_low(&mut m.freezes, v)?,
        "freezes_into" => set_into_low(&mut m.freezes, v, by_name)?,
        "condenses_at" => set_at_low(&mut m.condenses, v)?,
        "condenses_into" => set_into_low(&mut m.condenses, v, by_name)?,
        "lifetime" | "lasts_for" => {
            m.lifetime = opt_num(v)?.map(|x| x.clamp(1.0, 60000.0) as u16)
        }
        "decays_into" | "turns_into" => m.decays_into = name_to_id(v, by_name)?,
        "decays_into_chance" => m.decays_into_chance = num(v)?.clamp(0.0, 1.0),
        "flows" => m.flows = num(v)?.clamp(0.0, 16.0) as u8,
        "strength" => m.strength = num(v)?,
        "toughness" => m.toughness = opt_num(v)?.map(|t| t.max(0.0)),
        "breaks_into" => m.breaks_into = if is_none(v) { None } else { Some(name_to_id(v, by_name)?) },
        "friction" => m.friction = num(v)?.clamp(0.0, 2.0),
        "bounciness" => m.bounciness = num(v)?.clamp(0.0, 1.0),
        "explosion_size" => m.explosion_size = num(v)?.clamp(0.0, 64.0),
        "explosion_power" => m.explosion_power = num(v)?.max(0.0),
        "reaction" | "reactions" | "reacts" => {
            for part in v.split(';').filter(|p| !p.trim().is_empty()) {
                m.reactions.push(parse_reaction(part, by_name)?);
            }
        }
        _ => return Err(format!("unknown key `{key}` (see materials/README.md)")),
    }
    Ok(())
}

// Thresholds and targets can come in either order in the file, so a Change
// with only one half filled in uses a placeholder for the other half.
const UNSET_HIGH: f32 = f32::INFINITY;
const UNSET_LOW: f32 = f32::NEG_INFINITY;

fn set_at(c: &mut Option<Change>, v: &str) -> Result<(), String> {
    match opt_num(v)? {
        None => *c = None,
        Some(at) => c.get_or_insert(Change { at, into: AIR }).at = at,
    }
    Ok(())
}
fn set_into(c: &mut Option<Change>, v: &str, by: &HashMap<String, MatId>) -> Result<(), String> {
    let into = name_to_id(v, by)?;
    c.get_or_insert(Change { at: UNSET_HIGH, into }).into = into;
    Ok(())
}
fn set_at_low(c: &mut Option<Change>, v: &str) -> Result<(), String> {
    set_at(c, v)
}
fn set_into_low(c: &mut Option<Change>, v: &str, by: &HashMap<String, MatId>) -> Result<(), String> {
    let into = name_to_id(v, by)?;
    c.get_or_insert(Change { at: UNSET_LOW, into }).into = into;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_colours() {
        assert_eq!(strip_comment("color = #8b5a2b  # brown").trim(), "color = #8b5a2b");
        assert_eq!(strip_comment("# whole line"), "");
        assert_eq!(parse_color("#ff8000").unwrap(), [255, 128, 0]);
        assert_eq!(parse_color("1 2 3").unwrap(), [1, 2, 3]);
    }

    #[test]
    fn parses_a_material() {
        let m = Materials::from_texts(&[
            ("stone", "state = solid\n"),
            ("steam", "state = gas\n"),
            (
                "lava",
                "state = liquid\ndensity = 2600\ncolor = #ff6000\nfreezes_at = 700\nfreezes_into = stone\nreaction = water -> stone + steam, chance 0.5\n",
            ),
            ("water", "state = liquid\n"),
        ]);
        assert!(m.problems.is_empty(), "{:?}", m.problems);
        let lava = m.get(m.id("lava").unwrap());
        assert_eq!(lava.state, State::Liquid);
        assert_eq!(lava.freezes.unwrap().at, 700.0);
        assert_eq!(lava.freezes.unwrap().into, m.id("stone").unwrap());
        assert_eq!(lava.reactions.len(), 1);
        assert_eq!(lava.reactions[0].chance, 0.5);
    }

    #[test]
    fn reports_typos() {
        let m = Materials::from_texts(&[("sand", "densty = 5\nmelts_into = glas\n")]);
        assert_eq!(m.problems.len(), 2, "{:?}", m.problems);
    }
}
