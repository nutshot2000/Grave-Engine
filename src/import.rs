//! Bring in shapes from other programs.
//!
//! - **Images (.png)**: each pixel becomes a cell. Transparent pixels are empty.
//!   Colours pick the material whose `color` is closest, or use a mapping file
//!   next to the image (`crate.png` -> `crate.colors.txt`) with lines like
//!   `#8b5a2b = wood`.
//! - **Blender (.glb / .gltf)**: the model is seen from the front (Blender's
//!   front view, numpad 1) and turned into cells. Each Blender material's name
//!   picks the Grave material: a material called `wood` (or `Wood.001`, or
//!   `old_wood_planks`) becomes wood. One Blender metre is 10 cells by default.

use crate::material::{AIR, MatId, Materials, State, parse_color, strip_comment};
use std::path::Path;

/// A flat shape made of materials, ready to become an object or be painted.
#[derive(Clone, Debug)]
pub struct Shape {
    pub w: usize,
    pub h: usize,
    pub cells: Vec<Option<MatId>>,
}

impl Shape {
    pub fn count(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }
}

/// Load a .png, .glb or .gltf. `scale`: cells per pixel for images (whole
/// numbers), cells per metre for models. Returns the shape and any warnings.
pub fn load(path: &Path, mats: &Materials, scale: Option<f32>) -> Result<(Shape, Vec<String>), String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" => load_png(path, mats, scale.unwrap_or(1.0).round().max(1.0) as usize),
        "glb" | "gltf" => load_gltf(path, mats, scale.unwrap_or(10.0).max(0.1)),
        _ => Err(format!("{}: can only import .png, .glb and .gltf files", path.display())),
    }
}

fn read_png_rgba(path: &Path) -> Result<(usize, usize, Vec<u8>), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| format!("{}: {e}", path.display()))?;
    let size = reader.output_buffer_size().ok_or("image is too big")?;
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("{}: {e}", path.display()))?;
    let (w, h) = (info.width as usize, info.height as usize);
    let mut rgba = Vec::with_capacity(w * h * 4);
    let px = &buf[..info.buffer_size()];
    match info.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(px),
        png::ColorType::Rgb => px.chunks(3).for_each(|c| rgba.extend_from_slice(&[c[0], c[1], c[2], 255])),
        png::ColorType::GrayscaleAlpha => px.chunks(2).for_each(|c| rgba.extend_from_slice(&[c[0], c[0], c[0], c[1]])),
        png::ColorType::Grayscale => px.iter().for_each(|&g| rgba.extend_from_slice(&[g, g, g, 255])),
        png::ColorType::Indexed => return Err("indexed PNGs aren't supported, save as RGBA".into()),
    }
    Ok((w, h, rgba))
}

fn load_png(path: &Path, mats: &Materials, scale: usize) -> Result<(Shape, Vec<String>), String> {
    let (w, h, rgba) = read_png_rgba(path)?;
    let mut warnings = Vec::new();

    // Optional colour -> material mapping file.
    let mut mapping: Vec<([u8; 3], MatId)> = Vec::new();
    let map_path = path.with_extension("colors.txt");
    if let Ok(text) = std::fs::read_to_string(&map_path) {
        for (n, line) in text.lines().enumerate() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }
            let parsed = line.split_once('=').ok_or("expected `#rrggbb = material`".to_string()).and_then(|(c, m)| {
                let c = parse_color(c)?;
                let m = mats.id(m).ok_or(format!("no material called `{}`", m.trim()))?;
                Ok((c, m))
            });
            match parsed {
                Ok(p) => mapping.push(p),
                Err(e) => warnings.push(format!("{} line {}: {e}", map_path.display(), n + 1)),
            }
        }
    }
    // Otherwise, the closest material colour (not air or gases).
    let candidates: Vec<([u8; 3], MatId)> = mats
        .list
        .iter()
        .enumerate()
        .filter(|(i, m)| *i as MatId != AIR && m.state != State::Gas)
        .map(|(i, m)| (m.color, i as MatId))
        .collect();
    let dist = |a: [u8; 3], b: [u8; 3]| -> i32 {
        (0..3).map(|k| (a[k] as i32 - b[k] as i32).pow(2)).sum()
    };
    let pick = |c: [u8; 3]| -> MatId {
        if let Some(&(_, m)) = mapping.iter().filter(|(mc, _)| dist(*mc, c) <= 40 * 40).min_by_key(|(mc, _)| dist(*mc, c)) {
            return m;
        }
        candidates.iter().min_by_key(|(mc, _)| dist(*mc, c)).map(|&(_, m)| m).unwrap_or(AIR)
    };

    let (sw, sh) = (w * scale, h * scale);
    let mut cells = vec![None; sw * sh];
    let mut cache: std::collections::HashMap<[u8; 3], MatId> = std::collections::HashMap::new();
    for y in 0..h {
        for x in 0..w {
            let o = (y * w + x) * 4;
            if rgba[o + 3] < 128 {
                continue;
            }
            let c = [rgba[o], rgba[o + 1], rgba[o + 2]];
            let m = *cache.entry(c).or_insert_with(|| pick(c));
            for dy in 0..scale {
                for dx in 0..scale {
                    cells[(y * scale + dy) * sw + x * scale + dx] = Some(m);
                }
            }
        }
    }
    let shape = Shape { w: sw, h: sh, cells };
    if shape.count() == 0 {
        warnings.push(format!("{}: the image is completely transparent", path.display()));
    }
    Ok((shape, warnings))
}

/// Match a Blender material name like `Wood.001` or `old_wood_planks` to a material.
pub fn material_from_name(name: &str, mats: &Materials) -> Option<MatId> {
    let n = name.to_lowercase();
    let base = n.split('.').next().unwrap_or("").trim().replace([' ', '-'], "_");
    if let Some(id) = mats.id(&base) {
        return Some(id);
    }
    // Longest material name contained in the Blender name ("molten_iron" beats "iron").
    mats.list
        .iter()
        .enumerate()
        .filter(|(i, m)| *i as MatId != AIR && base.contains(&m.name))
        .max_by_key(|(_, m)| m.name.len())
        .map(|(i, _)| i as MatId)
}

fn mul(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    // Column-major 4x4 matrices, as glTF stores them.
    let mut r = [[0.0; 4]; 4];
    for c in 0..4 {
        for row in 0..4 {
            r[c][row] = (0..4).map(|k| a[k][row] * b[c][k]).sum();
        }
    }
    r
}

fn apply(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    let mut out = [0.0; 3];
    for row in 0..3 {
        out[row] = m[0][row] * p[0] + m[1][row] * p[1] + m[2][row] * p[2] + m[3][row];
    }
    out
}

fn load_gltf(path: &Path, mats: &Materials, cells_per_metre: f32) -> Result<(Shape, Vec<String>), String> {
    let (doc, buffers, _) = gltf::import(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut warnings = Vec::new();
    let fallback = mats.id("stone").unwrap_or(AIR);
    // Triangles in glTF space (x right, y up, z towards the front-view camera).
    let mut tris: Vec<([[f32; 3]; 3], MatId)> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();

    fn walk<'a>(node: gltf::Node<'a>, parent: [[f32; 4]; 4], out: &mut Vec<(gltf::Node<'a>, [[f32; 4]; 4])>) {
        let m = mul(&parent, &node.transform().matrix());
        out.push((node.clone(), m));
        for child in node.children() {
            walk(child, m, out);
        }
    }
    let identity = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
    let scene = doc.default_scene().or_else(|| doc.scenes().next()).ok_or("the file has no scene")?;
    let mut nodes = Vec::new();
    for n in scene.nodes() {
        walk(n, identity, &mut nodes);
    }
    for (node, m) in nodes {
        let Some(mesh) = node.mesh() else { continue };
        for prim in mesh.primitives() {
            if prim.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let mat_name = prim.material().name().unwrap_or("").to_string();
            let mat = match material_from_name(&mat_name, mats) {
                Some(id) => id,
                None => {
                    if !unknown.contains(&mat_name) {
                        unknown.push(mat_name.clone());
                    }
                    fallback
                }
            };
            let reader = prim.reader(|b| Some(&buffers[b.index()]));
            let Some(pos) = reader.read_positions() else { continue };
            let pos: Vec<[f32; 3]> = pos.map(|p| apply(&m, p)).collect();
            let idx: Vec<u32> = match reader.read_indices() {
                Some(i) => i.into_u32().collect(),
                None => (0..pos.len() as u32).collect(),
            };
            for t in idx.chunks_exact(3) {
                tris.push(([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]], mat));
            }
        }
    }
    for n in unknown {
        let shown = if n.is_empty() { "(no material)".to_string() } else { format!("`{n}`") };
        warnings.push(format!("{}: Blender material {shown} doesn't match a Grave material, using stone. Name it after a material, like `wood`.", path.display()));
    }
    if tris.is_empty() {
        return Err(format!("{}: no triangles found", path.display()));
    }

    // Front view: screen x = x, screen y = -y (down), nearest = largest z.
    let (mut minx, mut maxx, mut miny, mut maxy) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for (t, _) in &tris {
        for p in t {
            minx = minx.min(p[0]);
            maxx = maxx.max(p[0]);
            miny = miny.min(-p[1]);
            maxy = maxy.max(-p[1]);
        }
    }
    let s = cells_per_metre;
    let w = (((maxx - minx) * s).ceil() as usize).max(1);
    let h = (((maxy - miny) * s).ceil() as usize).max(1);
    if w * h > 2048 * 2048 {
        return Err(format!("{}: model would be {w}x{h} cells, too big (try a smaller scale)", path.display()));
    }
    let mut cells = vec![None; w * h];
    let mut depth = vec![f32::MIN; w * h];
    for (t, mat) in &tris {
        let p: Vec<(f32, f32, f32)> = t.iter().map(|p| ((p[0] - minx) * s, (-p[1] - miny) * s, p[2])).collect();
        let (a, b, c) = (p[0], p[1], p[2]);
        let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        if area.abs() < 1e-9 {
            continue;
        }
        let x0 = (a.0.min(b.0).min(c.0).floor().max(0.0)) as usize;
        let x1 = (a.0.max(b.0).max(c.0).ceil() as usize).min(w);
        let y0 = (a.1.min(b.1).min(c.1).floor().max(0.0)) as usize;
        let y1 = (a.1.max(b.1).max(c.1).ceil() as usize).min(h);
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((b.0 - px) * (c.1 - py) - (b.1 - py) * (c.0 - px)) / area;
                let w1 = ((c.0 - px) * (a.1 - py) - (c.1 - py) * (a.0 - px)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                    continue;
                }
                let z = w0 * a.2 + w1 * b.2 + w2 * c.2;
                let i = y * w + x;
                if z > depth[i] {
                    depth[i] = z;
                    cells[i] = Some(*mat);
                }
            }
        }
    }
    let shape = Shape { w, h, cells };
    if shape.count() == 0 {
        warnings.push(format!("{}: the model is too thin to show up at this scale", path.display()));
    }
    Ok((shape, warnings))
}
