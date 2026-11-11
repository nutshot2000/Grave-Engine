//! Material-grid assets shared by the workshop, scene files and AI tools.
//! One cell belongs to one named part; weld anchors and the pivot use grid coordinates.
use crate::{
    MatId, Materials, Sim,
    material::AIR,
    physics::{BodySpec, JointKind},
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
};
pub const MAX_SIZE: usize = 128;
pub const MAX_PARTS: usize = 24;
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub name: String,
    pub cells: Vec<MatId>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Weld {
    pub a: String,
    pub b: String,
    pub x: f32,
    pub y: f32,
    pub break_force: Option<f32>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub name: String,
    pub w: usize,
    pub h: usize,
    pub depth: f32,
    pub pivot: (f32, f32),
    pub parts: Vec<Part>,
    pub welds: Vec<Weld>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Brush {
    Pencil,
    Erase,
    Fill,
    Line,
    Rectangle,
    Circle,
}
pub fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 48
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
}
impl Asset {
    pub fn blank(name: &str, w: usize, h: usize) -> Result<Self, String> {
        if !valid_name(name) || w == 0 || h == 0 || w > MAX_SIZE || h > MAX_SIZE {
            return Err(
                "Use a short name with letters/numbers/-/_, and a grid from 1 to 128 cells.".into(),
            );
        }
        Ok(Self {
            name: name.into(),
            w,
            h,
            depth: 0.05,
            pivot: (w as f32 / 2., h as f32 / 2.),
            parts: vec![Part {
                name: "frame".into(),
                cells: vec![AIR; w * h],
            }],
            welds: vec![],
        })
    }
    pub fn add_part(&mut self, name: &str) -> Result<usize, String> {
        if !valid_name(name)
            || self.parts.iter().any(|p| p.name == name)
            || self.parts.len() >= MAX_PARTS
        {
            return Err("Part names must be unique, use letters/numbers/-/_, and there can be at most 24 parts.".into());
        }
        self.parts.push(Part {
            name: name.into(),
            cells: vec![AIR; self.w * self.h],
        });
        Ok(self.parts.len() - 1)
    }
    pub fn rename_part(&mut self, i: usize, name: &str) -> Result<(), String> {
        if i >= self.parts.len()
            || !valid_name(name)
            || self
                .parts
                .iter()
                .enumerate()
                .any(|(k, p)| k != i && p.name == name)
        {
            return Err("Choose a unique part name using letters/numbers/-/_.".into());
        }
        let old = self.parts[i].name.clone();
        self.parts[i].name = name.into();
        for j in &mut self.welds {
            if j.a == old {
                j.a = name.into();
            }
            if j.b == old {
                j.b = name.into();
            }
        }
        Ok(())
    }
    pub fn remove_part(&mut self, i: usize) {
        if self.parts.len() <= 1 || i >= self.parts.len() {
            return;
        }
        let name = self.parts.remove(i).name;
        self.welds.retain(|j| j.a != name && j.b != name);
    }
    pub fn resize(&mut self, w: usize, h: usize) -> Result<(), String> {
        if w == 0 || h == 0 || w > MAX_SIZE || h > MAX_SIZE {
            return Err("Grid dimensions must be 1–128.".into());
        }
        for p in &mut self.parts {
            let mut cells = vec![AIR; w * h];
            for y in 0..h.min(self.h) {
                for x in 0..w.min(self.w) {
                    cells[y * w + x] = p.cells[y * self.w + x];
                }
            }
            p.cells = cells;
        }
        self.w = w;
        self.h = h;
        self.pivot.0 = self.pivot.0.clamp(0., w as f32);
        self.pivot.1 = self.pivot.1.clamp(0., h as f32);
        self.welds.retain(|j| j.x <= w as f32 && j.y <= h as f32);
        Ok(())
    }
    pub fn put(&mut self, part: usize, x: i32, y: i32, mat: MatId) {
        if part >= self.parts.len() || x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = y as usize * self.w + x as usize;
        if mat != AIR {
            for (k, p) in self.parts.iter_mut().enumerate() {
                if k != part {
                    p.cells[i] = AIR;
                }
            }
        }
        self.parts[part].cells[i] = mat;
    }
    pub fn draw(&mut self, part: usize, brush: Brush, a: (i32, i32), b: (i32, i32), mat: MatId) {
        if part >= self.parts.len() {
            return;
        }
        let mat = if brush == Brush::Erase { AIR } else { mat };
        if brush == Brush::Fill {
            if a.0 < 0 || a.1 < 0 || a.0 >= self.w as i32 || a.1 >= self.h as i32 {
                return;
            }
            let original = self.parts[part].cells[a.1 as usize * self.w + a.0 as usize];
            if original == mat {
                return;
            }
            let composite = self.composite();
            let mut q = VecDeque::from([a]);
            let mut seen = vec![false; self.w * self.h];
            while let Some((x, y)) = q.pop_front() {
                if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
                    continue;
                }
                let i = y as usize * self.w + x as usize;
                if seen[i]
                    || self.parts[part].cells[i] != original
                    || (original == AIR && composite[i] != AIR)
                {
                    continue;
                }
                seen[i] = true;
                self.put(part, x, y, mat);
                q.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
            }
            return;
        }
        if a.0.max(b.0) < 0
            || a.1.max(b.1) < 0
            || a.0.min(b.0) >= self.w as i32
            || a.1.min(b.1) >= self.h as i32
        {
            return;
        }
        let (x0, x1, y0, y1) = (
            a.0.min(b.0).clamp(0, self.w as i32 - 1),
            a.0.max(b.0).clamp(0, self.w as i32 - 1),
            a.1.min(b.1).clamp(0, self.h as i32 - 1),
            a.1.max(b.1).clamp(0, self.h as i32 - 1),
        );
        match brush {
            Brush::Rectangle => {
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        self.put(part, x, y, mat);
                    }
                }
            }
            Brush::Circle => {
                let (cx, cy) = ((a.0 + b.0) as f32 / 2., (a.1 + b.1) as f32 / 2.);
                let (rx, ry) = (
                    (((b.0 - a.0).abs() + 1) as f32 / 2.).max(0.5),
                    ((((b.1 - a.1).abs() + 1) as f32 / 2.).max(0.5)),
                );
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        if ((x as f32 - cx) / rx).powi(2) + ((y as f32 - cy) / ry).powi(2) <= 1. {
                            self.put(part, x, y, mat);
                        }
                    }
                }
            }
            _ => {
                let n = (b.0 - a.0).abs().max((b.1 - a.1).abs()).min(512).max(1);
                for k in 0..=n {
                    self.put(
                        part,
                        a.0 + (b.0 - a.0) * k / n,
                        a.1 + (b.1 - a.1) * k / n,
                        mat,
                    );
                }
            }
        }
    }
    pub fn move_part(&mut self, part: usize, dx: i32, dy: i32) {
        if part >= self.parts.len() {
            return;
        }
        let old = self.parts[part].cells.clone();
        self.parts[part].cells.fill(AIR);
        for y in 0..self.h {
            for x in 0..self.w {
                let m = old[y * self.w + x];
                if m != AIR {
                    self.put(part, x as i32 + dx, y as i32 + dy, m);
                }
            }
        }
        let name = &self.parts[part].name;
        for j in &mut self.welds {
            if &j.a == name || &j.b == name {
                j.x = (j.x + dx as f32).clamp(0., self.w as f32);
                j.y = (j.y + dy as f32).clamp(0., self.h as f32);
            }
        }
    }
    pub fn composite(&self) -> Vec<MatId> {
        let mut cells = vec![AIR; self.w * self.h];
        for p in &self.parts {
            for (out, m) in cells.iter_mut().zip(&p.cells) {
                if *m != AIR {
                    *out = *m;
                }
            }
        }
        cells
    }
    pub fn validate(&self, mats: &Materials) -> Vec<String> {
        let mut out = vec![];
        let mut owners = vec![false; self.w * self.h];
        let mut names = BTreeSet::new();
        if !valid_name(&self.name) {
            out.push("Asset name needs letters/numbers/-/_.".into());
        }
        if !self.depth.is_finite() || self.depth <= 0. || self.depth > 10. {
            out.push("Depth must be greater than zero and at most 10 metres.".into());
        }
        for p in &self.parts {
            if !valid_name(&p.name) || !names.insert(p.name.clone()) {
                out.push(format!("Invalid or duplicate part name: {}", p.name));
            }
            if p.cells.len() != owners.len() {
                out.push(format!("{} has the wrong grid size", p.name));
                continue;
            }
            if p.cells.iter().all(|m| *m == AIR) {
                out.push(format!("{} is empty", p.name));
            }
            for (i, m) in p.cells.iter().enumerate() {
                if *m as usize >= mats.list.len() {
                    out.push(format!("{} has an unknown material", p.name));
                    break;
                }
                if *m != AIR {
                    if owners[i] {
                        out.push("Two parts overlap. Each cell must belong to one part.".into());
                        break;
                    }
                    owners[i] = true;
                }
            }
            let mut visited = vec![false; p.cells.len()];
            let mut islands = 0;
            for i in 0..p.cells.len() {
                if p.cells[i] == AIR || visited[i] {
                    continue;
                }
                islands += 1;
                let mut q = vec![i];
                visited[i] = true;
                while let Some(k) = q.pop() {
                    let (x, y) = (k % self.w, k / self.w);
                    for n in [
                        if x > 0 { Some(k - 1) } else { None },
                        if x + 1 < self.w { Some(k + 1) } else { None },
                        if y > 0 { Some(k - self.w) } else { None },
                        if y + 1 < self.h {
                            Some(k + self.w)
                        } else {
                            None
                        },
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if !visited[n] && p.cells[n] != AIR {
                            visited[n] = true;
                            q.push(n);
                        }
                    }
                }
            }
            if islands > 1 {
                out.push(format!(
                    "{} has {islands} disconnected islands; it may split in simulation",
                    p.name
                ));
            }
        }
        for j in &self.welds {
            if j.a == j.b || !names.contains(&j.a) || !names.contains(&j.b) {
                out.push(format!(
                    "Weld {} → {} has missing or identical parts",
                    j.a, j.b
                ));
            }
        }
        if self.parts.len() > 1 && self.welds.is_empty() {
            out.push(
                "Parts are unjoined and will fall separately. Add welds to assemble them.".into(),
            );
        }
        out
    }
    pub fn bodies(&self, mats: &Materials, x: f32, y: f32) -> Vec<(String, BodySpec)> {
        self.parts
            .iter()
            .filter(|p| p.cells.iter().any(|m| *m != AIR))
            .map(|p| {
                let cells: Vec<_> = p
                    .cells
                    .iter()
                    .map(|m| if *m == AIR { None } else { Some(*m) })
                    .collect();
                let mut b = BodySpec::from_mats(
                    self.w,
                    self.h,
                    &cells,
                    mats,
                    x + self.w as f32 / 2. - self.pivot.0,
                    y + self.h as f32 / 2. - self.pivot.1,
                );
                b.depth = self.depth;
                (p.name.clone(), b)
            })
            .collect()
    }
    pub fn instantiate(
        &self,
        sim: &mut Sim,
        mats: &Materials,
        x: f32,
        y: f32,
    ) -> Result<Vec<(String, u64)>, String> {
        let specs = self.bodies(mats, x, y);
        if specs.is_empty() {
            return Err("Draw some material before placing this asset.".into());
        }
        let mut ids = vec![];
        for (name, b) in specs {
            let id = sim
                .physics
                .add_exact(&mut sim.world, mats, b)
                .ok_or("Couldn't create asset body")?;
            ids.push((name, id));
        }
        for j in &self.welds {
            let a = ids.iter().find(|(n, _)| n == &j.a).map(|(_, id)| *id);
            let b = ids.iter().find(|(n, _)| n == &j.b).map(|(_, id)| *id);
            if let (Some(a), Some(b)) = (a, b) {
                sim.physics.join(
                    mats,
                    a,
                    b,
                    JointKind::Weld,
                    (x + j.x - self.pivot.0, y + j.y - self.pivot.1),
                    None,
                    None,
                );
                if let Some(f) = j.break_force {
                    sim.physics.set_joint_break_force(a, b, f);
                }
            }
        }
        Ok(ids)
    }
    pub fn to_text(&self, mats: &Materials) -> String {
        let mut out = format!(
            "# Grave material asset. One cell = 10 cm. Named parts share a local grid.\nformat 1\nasset {} {} {}\ndepth {}\npivot {} {}\n",
            self.name, self.w, self.h, self.depth, self.pivot.0, self.pivot.1
        );
        let chars:Vec<_>="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789%&*+=?@$~^!:;<>/|()[]{}-_,".chars().collect();
        let used: BTreeSet<_> = self
            .parts
            .iter()
            .flat_map(|p| p.cells.iter().copied())
            .filter(|m| *m != AIR)
            .collect();
        let legend: BTreeMap<_, _> = used.into_iter().zip(chars).collect();
        for (m, ch) in &legend {
            out.push_str(&format!("legend {ch} {}\n", mats.get(*m).name));
        }
        for p in &self.parts {
            out.push_str(&format!("\npart {}\ngrid\n", p.name));
            for row in p.cells.chunks(self.w) {
                for m in row {
                    out.push(if *m == AIR { '.' } else { legend[m] });
                }
                out.push('\n');
            }
            out.push_str("end\n");
        }
        for j in &self.welds {
            out.push_str(&format!(
                "weld {} {} {} {} {}\n",
                j.a,
                j.b,
                j.x,
                j.y,
                j.break_force.unwrap_or(0.)
            ));
        }
        out
    }
    pub fn scene_text(&self, mats: &Materials, x: f32, y: f32) -> String {
        let mut out = String::from("fill air\n");
        let chars:Vec<_>="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789%&*+=?@$~^!:;<>/|()[]{}-_,".chars().collect();
        let used: BTreeSet<_> = self
            .parts
            .iter()
            .flat_map(|p| p.cells.iter().copied())
            .filter(|m| *m != AIR)
            .collect();
        let legend: BTreeMap<_, _> = used.into_iter().zip(chars).collect();
        for (m, ch) in &legend {
            out.push_str(&format!("legend {ch} {}\n", mats.get(*m).name));
        }
        for (name, b) in self.bodies(mats, x, y) {
            out.push_str(&format!("body {} {} 0\n", b.x, b.y));
            for row in b.cells.chunks(b.w) {
                for c in row {
                    out.push(c.map_or('.', |c| legend[&c.mat]));
                }
                out.push('\n');
            }
            out.push_str(&format!("end\nname {name}\ndepth {}\n", b.depth));
        }
        for j in &self.welds {
            out.push_str(&format!(
                "weld {} {} at {} {}",
                j.a,
                j.b,
                x + j.x - self.pivot.0,
                y + j.y - self.pivot.1
            ));
            if let Some(f) = j.break_force {
                out.push_str(&format!(" break {f}"));
            }
            out.push('\n');
        }
        out
    }
    pub fn save(&self, path: &Path, mats: &Materials) -> Result<(), String> {
        std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))
            .map_err(|e| e.to_string())?;
        std::fs::write(path, self.to_text(mats)).map_err(|e| e.to_string())
    }
    pub fn load_file(path: &Path, mats: &Materials) -> Result<Self, String> {
        let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if meta.len() > 8 * 1024 * 1024 {
            return Err("Asset file exceeds 8 MB.".into());
        }
        Self::parse(
            &std::fs::read_to_string(path).map_err(|e| e.to_string())?,
            mats,
        )
    }
    pub fn parse(text: &str, mats: &Materials) -> Result<Self, String> {
        let mut asset: Option<Self> = None;
        let mut legend = BTreeMap::from([('.', AIR)]);
        let mut selected = 0;
        let mut first = true;
        let mut rows = text.lines().enumerate();
        while let Some((ln, raw)) = rows.next() {
            let line = raw.trim().trim_start_matches('\u{feff}');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let v: Vec<_> = line.split_whitespace().collect();
            let err = |s: &str| format!("Asset line {}: {s}", ln + 1);
            let f = |i: usize| {
                v.get(i)
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|x| x.is_finite())
                    .ok_or_else(|| err("missing or invalid number"))
            };
            let int = |i: usize| {
                v.get(i)
                    .and_then(|s| s.parse::<i32>().ok())
                    .filter(|x| x.unsigned_abs() <= 4096)
                    .ok_or_else(|| err("integer must be between -4096 and 4096"))
            };
            if v[0] == "format" {
                if v.get(1) != Some(&"1") {
                    return Err(err("unsupported format version"));
                }
                continue;
            }
            if v[0] == "asset" {
                if asset.is_some() {
                    return Err(err("duplicate asset header"));
                }
                let (w, h) = (int(2)?, int(3)?);
                if w < 1 || h < 1 {
                    return Err(err("grid dimensions must be positive"));
                }
                asset = Some(Self::blank(
                    v.get(1).ok_or_else(|| err("missing asset name"))?,
                    w as usize,
                    h as usize,
                )?);
                continue;
            }
            let a = asset
                .as_mut()
                .ok_or_else(|| err("start with: asset NAME WIDTH HEIGHT"))?;
            match v[0] {
                "depth" => {
                    let d = f(1)?;
                    if d <= 0. || d > 10. {
                        return Err(err("depth must be 0–10 metres, excluding zero"));
                    }
                    a.depth = d;
                }
                "pivot" => {
                    let p = (f(1)?, f(2)?);
                    if p.0 < 0. || p.1 < 0. || p.0 > a.w as f32 || p.1 > a.h as f32 {
                        return Err(err("pivot must be inside the grid"));
                    }
                    a.pivot = p;
                }
                "legend" => {
                    let ch = v
                        .get(1)
                        .and_then(|s| {
                            if s.chars().count() == 1 {
                                s.chars().next()
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| err("legend needs a single character"))?;
                    let m = mats
                        .id(v.get(2).ok_or_else(|| err("legend needs a material"))?)
                        .ok_or_else(|| err("unknown material"))?;
                    if ch == '.' && m != AIR {
                        return Err(err("dot is reserved for empty cells"));
                    }
                    legend.insert(ch, m);
                }
                "part" => {
                    let n = *v.get(1).ok_or_else(|| err("missing part name"))?;
                    if first {
                        a.rename_part(0, n)?;
                        selected = 0;
                        first = false;
                    } else {
                        selected = a.add_part(n)?;
                    }
                }
                "grid" => {
                    let mut ended = false;
                    let mut y = 0;
                    for (_, row) in rows.by_ref() {
                        if row.trim() == "end" {
                            ended = true;
                            break;
                        }
                        let row = row.trim();
                        if y >= a.h || row.chars().count() != a.w {
                            return Err(err("grid rows must match asset width and height"));
                        }
                        for (x, ch) in row.chars().enumerate() {
                            let m = *legend
                                .get(&ch)
                                .ok_or_else(|| err("undefined grid character"))?;
                            if m != AIR {
                                let i = y * a.w + x;
                                if a.parts
                                    .iter()
                                    .enumerate()
                                    .any(|(k, p)| k != selected && p.cells[i] != AIR)
                                {
                                    return Err(err("parts overlap at the same cell"));
                                }
                            }
                            a.parts[selected].cells[y * a.w + x] = m;
                        }
                        y += 1;
                    }
                    if !ended || y != a.h {
                        return Err(err("grid needs exactly HEIGHT rows followed by end"));
                    }
                }
                "rect" | "line" | "circle" | "fill" => {
                    let m = mats
                        .id(v.get(1).ok_or_else(|| err("missing material"))?)
                        .ok_or_else(|| err("unknown material"))?;
                    match v[0] {
                        "fill" => a.draw(
                            selected,
                            Brush::Rectangle,
                            (0, 0),
                            (a.w as i32 - 1, a.h as i32 - 1),
                            m,
                        ),
                        "rect" => {
                            let (x, y, w, h) = (int(2)?, int(3)?, int(4)?, int(5)?);
                            if w <= 0 || h <= 0 {
                                return Err(err("rectangle width/height must be positive"));
                            }
                            a.draw(
                                selected,
                                Brush::Rectangle,
                                (x, y),
                                (x + w - 1, y + h - 1),
                                m,
                            );
                        }
                        "line" => a.draw(
                            selected,
                            Brush::Line,
                            (int(2)?, int(3)?),
                            (int(4)?, int(5)?),
                            m,
                        ),
                        _ => {
                            let (x, y, r) = (int(2)?, int(3)?, int(4)?);
                            if r < 0 {
                                return Err(err("circle radius must be nonnegative"));
                            }
                            a.draw(selected, Brush::Circle, (x - r, y - r), (x + r, y + r), m);
                        }
                    }
                }
                "weld" => {
                    let x = f(3)?;
                    let y = f(4)?;
                    let force = f(5)?;
                    if x < 0. || y < 0. || x > a.w as f32 || y > a.h as f32 || force < 0. {
                        return Err(err(
                            "weld anchor must be inside the grid, force nonnegative",
                        ));
                    }
                    a.welds.push(Weld {
                        a: v.get(1)
                            .ok_or_else(|| err("missing first part"))?
                            .to_string(),
                        b: v.get(2)
                            .ok_or_else(|| err("missing second part"))?
                            .to_string(),
                        x,
                        y,
                        break_force: if force == 0. { None } else { Some(force) },
                    });
                }
                _ => return Err(err("unknown asset command")),
            }
        }
        let a = asset.ok_or("Asset header is missing")?;
        let names: BTreeSet<_> = a.parts.iter().map(|p| p.name.as_str()).collect();
        for j in &a.welds {
            if j.a == j.b || !names.contains(j.a.as_str()) || !names.contains(j.b.as_str()) {
                return Err("Weld references missing or identical parts.".into());
            }
        }
        Ok(a)
    }
}
