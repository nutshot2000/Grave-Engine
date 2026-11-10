//! Bring in shapes from other programs.
//!
//! - **Images** (.png .jpg .bmp .tga): each pixel becomes a cell; transparent
//!   pixels are empty. Colours pick the material whose `color` is closest, or
//!   use a mapping file next to the image (`crate.png` -> `crate.colors.txt`)
//!   with lines like `#8b5a2b = wood`.
//! - **3D models** (.glb .gltf .obj, and .fbx .blend through Blender): the
//!   model is seen from the front (Blender's front view, numpad 1) and turned
//!   into cells. For each part:
//!   1. a material *named* after a Grave material (`wood`, `Wood.001`,
//!      `old_wood_planks`) becomes that material;
//!   2. otherwise its texture picture, if it has one, is sampled pixel by
//!      pixel and each colour picks the closest material;
//!   3. otherwise its plain colour picks the closest material.
//!
//!   One metre in the model is 10 cells by default.

use crate::material::{AIR, MatId, Materials, State, parse_color, strip_comment};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

/// File types `load` understands.
pub const KINDS: &[&str] = &["png", "jpg", "jpeg", "bmp", "tga", "glb", "gltf", "obj", "fbx", "blend"];

/// Load an image or model. `scale`: cells per pixel for images (whole
/// numbers), cells per metre for models. Returns the shape and any warnings.
pub fn load(path: &Path, mats: &Materials, scale: Option<f32>) -> Result<(Shape, Vec<String>), String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let per_metre = scale.unwrap_or(10.0).max(0.1);
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "bmp" | "tga" => load_image(path, mats, scale.unwrap_or(1.0).round().max(1.0) as usize),
        "glb" | "gltf" => load_gltf(path, mats, per_metre),
        "obj" => load_obj(path, mats, per_metre),
        "fbx" | "blend" => {
            let glb = via_blender(path)?;
            load_gltf(&glb, mats, per_metre)
        }
        _ => Err(format!("{}: can't import this kind of file (try {})", path.display(), KINDS.join(", "))),
    }
}

// ----------------------------------------------------------------- colours --

/// Picks a material for a colour: a mapping file first (if there is one next
/// to the imported file), otherwise the material with the closest `color`.
struct ColorMap {
    mapping: Vec<([u8; 3], MatId)>,
    candidates: Vec<([u8; 3], MatId, bool)>,
    cache: HashMap<([u8; 3], Option<bool>), MatId>,
}

impl ColorMap {
    /// `solid_only`: only pick solid materials (models become objects, so a
    /// blue part shouldn't turn into water).
    fn new(path: &Path, mats: &Materials, warnings: &mut Vec<String>, solid_only: bool) -> Self {
        let mut mapping = Vec::new();
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
        let candidates = mats
            .list
            .iter()
            .enumerate()
            .filter(|(i, m)| *i as MatId != AIR && if solid_only { m.state == State::Solid } else { m.state != State::Gas })
            // Only things that last at room temperature (no ice, no lava).
            .filter(|(_, m)| {
                let room = crate::world::ROOM_TEMP + 15.0;
                m.melts.is_none_or(|c| c.at > room)
                    && m.boils.is_none_or(|c| c.at > room)
                    && m.freezes.is_none_or(|c| c.at < crate::world::ROOM_TEMP - 15.0)
                    && m.lifetime.is_none()
            })
            .map(|(i, m)| (m.color, i as MatId, m.metal))
            .collect();
        ColorMap { mapping, candidates, cache: HashMap::new() }
    }

    /// `metal`: Some(true) = the part is metallic (pick a metal), Some(false)
    /// = it isn't (don't), None = don't know (pictures).
    fn pick(&mut self, c: [u8; 3], metal: Option<bool>) -> MatId {
        if let Some(&m) = self.cache.get(&(c, metal)) {
            return m;
        }
        let dist = |a: [u8; 3], b: [u8; 3]| -> i32 { (0..3).map(|k| (a[k] as i32 - b[k] as i32).pow(2)).sum() };
        let m = self
            .mapping
            .iter()
            .filter(|(mc, _)| dist(*mc, c) <= 40 * 40)
            .min_by_key(|(mc, _)| dist(*mc, c))
            .map(|&(_, m)| m)
            .or_else(|| {
                let fits = |is_metal: bool| metal.is_none_or(|want| want == is_metal);
                let best = |only_fitting: bool| {
                    self.candidates.iter().filter(|(_, _, is_metal)| !only_fitting || fits(*is_metal)).min_by_key(|(mc, _, _)| dist(*mc, c)).map(|&(_, m, _)| m)
                };
                // (No metals at all? Then the closest colour of anything.)
                best(true).or_else(|| best(false))
            })
            .unwrap_or(AIR);
        self.cache.insert((c, metal), m);
        m
    }
}

// ------------------------------------------------------------------ images --

/// A picture as RGBA bytes.
#[derive(Clone)]
pub struct Picture {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

impl Picture {
    pub fn open(path: &Path) -> Result<Picture, String> {
        let img = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?.to_rgba8();
        Ok(Picture { w: img.width() as usize, h: img.height() as usize, rgba: img.into_raw() })
    }

    /// Colour and alpha at texture coordinates (wrapping, like a tiled texture).
    pub fn sample(&self, u: f32, v: f32) -> ([u8; 3], u8) {
        let x = ((u.rem_euclid(1.0)) * self.w as f32) as usize % self.w.max(1);
        let y = ((v.rem_euclid(1.0)) * self.h as f32) as usize % self.h.max(1);
        let o = (y * self.w + x) * 4;
        ([self.rgba[o], self.rgba[o + 1], self.rgba[o + 2]], self.rgba[o + 3])
    }
}

fn load_image(path: &Path, mats: &Materials, scale: usize) -> Result<(Shape, Vec<String>), String> {
    let pic = Picture::open(path)?;
    let mut warnings = Vec::new();
    let mut colors = ColorMap::new(path, mats, &mut warnings, false);
    let (sw, sh) = (pic.w * scale, pic.h * scale);
    let mut cells = vec![None; sw * sh];
    for y in 0..pic.h {
        for x in 0..pic.w {
            let o = (y * pic.w + x) * 4;
            if pic.rgba[o + 3] < 128 {
                continue;
            }
            let m = colors.pick([pic.rgba[o], pic.rgba[o + 1], pic.rgba[o + 2]], None);
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

/// Match a material name like `Wood.001` or `old_wood_planks` to a material.
pub fn material_from_name(name: &str, mats: &Materials) -> Option<MatId> {
    let n = name.to_lowercase();
    let base = n.split('.').next().unwrap_or("").trim().replace([' ', '-'], "_");
    if let Some(id) = mats.id(&base) {
        return Some(id);
    }
    // Longest material name contained in the given name ("molten_iron" beats "iron").
    mats.list
        .iter()
        .enumerate()
        .filter(|(i, m)| *i as MatId != AIR && base.contains(&m.name))
        .max_by_key(|(_, m)| m.name.len())
        .map(|(i, _)| i as MatId)
}

// -------------------------------------------------------- triangles -> cells --

/// How one part of a model is coloured in.
#[derive(Clone)]
enum Paint {
    /// Named after a Grave material.
    Mat(MatId),
    /// A plain colour (and whether the part is metallic, if known).
    Color([u8; 3], Option<bool>),
    /// A texture picture (index into the pictures list), tinted.
    Texture(usize, [f32; 3], Option<bool>),
}

struct Tri {
    /// Model space: x right, y up, z towards the front-view camera.
    p: [[f32; 3]; 3],
    uv: [[f32; 2]; 3],
    paint: Paint,
}

/// Look at triangles from the front and turn them into cells.
fn rasterize(tris: &[Tri], pictures: &[Picture], colors: &mut ColorMap, per_metre: f32, path: &Path) -> Result<Shape, String> {
    if tris.is_empty() {
        return Err(format!("{}: no triangles found", path.display()));
    }
    let (mut minx, mut maxx, mut miny, mut maxy) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for t in tris {
        for p in &t.p {
            minx = minx.min(p[0]);
            maxx = maxx.max(p[0]);
            miny = miny.min(-p[1]);
            maxy = maxy.max(-p[1]);
        }
    }
    let s = per_metre;
    let w = (((maxx - minx) * s).ceil() as usize).max(1);
    let h = (((maxy - miny) * s).ceil() as usize).max(1);
    if w * h > 2048 * 2048 {
        return Err(format!("{}: the model would be {w}x{h} cells, too big (try a smaller scale)", path.display()));
    }
    let mut cells = vec![None; w * h];
    let mut depth = vec![f32::MIN; w * h];
    for t in tris {
        let p: Vec<(f32, f32, f32)> = t.p.iter().map(|p| ((p[0] - minx) * s, (-p[1] - miny) * s, p[2])).collect();
        let (a, b, c) = (p[0], p[1], p[2]);
        let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        if area.abs() < 1e-9 {
            continue;
        }
        // Parts thinner than a cell still leave a line of cells: test the
        // triangle's own middle too, not just cell centres.
        let mid = ((a.0 + b.0 + c.0) / 3.0, (a.1 + b.1 + c.1) / 3.0);
        let mut wrote = false;
        // Thin slivers (thinner than a cell): count any cell centre within
        // half a cell of the triangle.
        let thin = (a.0.max(b.0).max(c.0) - a.0.min(b.0).min(c.0)) < 1.5 || (a.1.max(b.1).max(c.1) - a.1.min(b.1).min(c.1)) < 1.5;
        let x0 = (a.0.min(b.0).min(c.0) - 0.5).floor().max(0.0) as usize;
        let x1 = ((a.0.max(b.0).max(c.0) + 0.5).ceil() as usize).min(w);
        let y0 = (a.1.min(b.1).min(c.1) - 0.5).floor().max(0.0) as usize;
        let y1 = ((a.1.max(b.1).max(c.1) + 0.5).ceil() as usize).min(h);
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((b.0 - px) * (c.1 - py) - (b.1 - py) * (c.0 - px)) / area;
                let w1 = ((c.0 - px) * (a.1 - py) - (c.1 - py) * (a.0 - px)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                    let near = thin && [(a, b), (b, c), (c, a)].iter().any(|(p, q)| seg_dist((px, py), (p.0, p.1), (q.0, q.1)) <= 0.5);
                    if !near {
                        continue;
                    }
                }
                let (w0, w1, w2) = (w0.clamp(0.0, 1.0), w1.clamp(0.0, 1.0), w2.clamp(0.0, 1.0));
                let z = w0 * a.2 + w1 * b.2 + w2 * c.2;
                let i = y * w + x;
                if z <= depth[i] {
                    continue;
                }
                let mat = match &t.paint {
                    Paint::Mat(m) => Some(*m),
                    Paint::Color(c, metal) => Some(colors.pick(*c, *metal)),
                    Paint::Texture(k, tint, metal) => {
                        let u = w0 * t.uv[0][0] + w1 * t.uv[1][0] + w2 * t.uv[2][0];
                        let v = w0 * t.uv[0][1] + w1 * t.uv[1][1] + w2 * t.uv[2][1];
                        let (c, alpha) = pictures[*k].sample(u, v);
                        (alpha >= 128).then(|| {
                            let c = [0, 1, 2].map(|j| (c[j] as f32 * tint[j]).clamp(0.0, 255.0) as u8);
                            colors.pick(c, *metal)
                        })
                    }
                };
                if let Some(m) = mat {
                    depth[i] = z;
                    cells[i] = Some(m);
                    wrote = true;
                }
            }
        }
        if !wrote {
            let (mx, my) = (mid.0.floor() as usize, mid.1.floor() as usize);
            if mx < w && my < h {
                let i = my * w + mx;
                let z = (a.2 + b.2 + c.2) / 3.0;
                let mat = match &t.paint {
                    Paint::Mat(m) => Some(*m),
                    Paint::Color(c, metal) => Some(colors.pick(*c, *metal)),
                    Paint::Texture(k, tint, metal) => {
                        let u = (t.uv[0][0] + t.uv[1][0] + t.uv[2][0]) / 3.0;
                        let v = (t.uv[0][1] + t.uv[1][1] + t.uv[2][1]) / 3.0;
                        let (c, alpha) = pictures[*k].sample(u, v);
                        (alpha >= 128).then(|| colors.pick([0, 1, 2].map(|j| (c[j] as f32 * tint[j]).clamp(0.0, 255.0) as u8), *metal))
                    }
                };
                if let (Some(m), true) = (mat, z > depth[i]) {
                    depth[i] = z;
                    cells[i] = Some(m);
                }
            }
        }
    }
    Ok(Shape { w, h, cells })
}

/// Distance from a point to a line segment.
fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
}

fn to_u8(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Decide how a part is coloured in. Plain white with no texture and an
/// unknown name: stone, with a warning.
fn choose_paint(name: &str, color: [f32; 3], texture: Option<usize>, metal: Option<bool>, mats: &Materials, unknown: &mut Vec<String>) -> Paint {
    if let Some(id) = material_from_name(name, mats) {
        return Paint::Mat(id);
    }
    if let Some(k) = texture {
        return Paint::Texture(k, color, metal);
    }
    let white = color.iter().all(|&c| c > 0.97);
    if !white || metal == Some(true) {
        return Paint::Color(to_u8(color), metal);
    }
    if !unknown.contains(&name.to_string()) {
        unknown.push(name.to_string());
    }
    Paint::Mat(mats.id("stone").unwrap_or(AIR))
}

fn unknown_warnings(path: &Path, unknown: Vec<String>) -> Vec<String> {
    unknown
        .into_iter()
        .map(|n| {
            let shown = if n.is_empty() { "(no material)".to_string() } else { format!("`{n}`") };
            format!(
                "{}: material {shown} has no Grave material name, colour or texture, so it's stone. Name it after a material, like `wood`.",
                path.display()
            )
        })
        .collect()
}

// -------------------------------------------------------------------- glTF --

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

fn gltf_picture(d: &gltf::image::Data) -> Picture {
    use gltf::image::Format;
    let (w, h) = (d.width as usize, d.height as usize);
    let px = &d.pixels;
    let rgba: Vec<u8> = match d.format {
        Format::R8G8B8A8 => px.clone(),
        Format::R8G8B8 => px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        Format::R8G8 => px.chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        Format::R8 => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        // 16-bit and float pictures: keep the high bytes.
        Format::R16G16B16A16 => px.chunks(8).flat_map(|c| [c[1], c[3], c[5], c[7]]).collect(),
        Format::R16G16B16 => px.chunks(6).flat_map(|c| [c[1], c[3], c[5], 255]).collect(),
        _ => vec![200; w * h * 4],
    };
    Picture { w, h, rgba }
}

fn load_gltf(path: &Path, mats: &Materials, per_metre: f32) -> Result<(Shape, Vec<String>), String> {
    let (doc, buffers, images) = gltf::import(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut warnings = Vec::new();
    let pictures: Vec<Picture> = images.iter().map(gltf_picture).collect();
    let mut tris = Vec::new();
    let mut unknown = Vec::new();

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
            let material = prim.material();
            let pbr = material.pbr_metallic_roughness();
            let f = pbr.base_color_factor();
            let tex = pbr.base_color_texture();
            let tex_set = tex.as_ref().map(|t| t.tex_coord()).unwrap_or(0);
            let paint = choose_paint(
                material.name().unwrap_or(""),
                [f[0], f[1], f[2]],
                tex.map(|t| t.texture().source().index()),
                Some(pbr.metallic_factor() >= 0.5),
                mats,
                &mut unknown,
            );
            let reader = prim.reader(|b| Some(&buffers[b.index()]));
            let Some(pos) = reader.read_positions() else { continue };
            let pos: Vec<[f32; 3]> = pos.map(|p| apply(&m, p)).collect();
            let uvs: Vec<[f32; 2]> = reader.read_tex_coords(tex_set).map(|t| t.into_f32().collect()).unwrap_or_else(|| vec![[0.0, 0.0]; pos.len()]);
            let idx: Vec<u32> = match reader.read_indices() {
                Some(i) => i.into_u32().collect(),
                None => (0..pos.len() as u32).collect(),
            };
            for t in idx.chunks_exact(3) {
                let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
                tris.push(Tri { p: [pos[a], pos[b], pos[c]], uv: [uvs[a], uvs[b], uvs[c]], paint: paint.clone() });
            }
        }
    }
    warnings.extend(unknown_warnings(path, unknown));
    let mut colors = ColorMap::new(path, mats, &mut warnings, true);
    let shape = rasterize(&tris, &pictures, &mut colors, per_metre, path)?;
    if shape.count() == 0 {
        warnings.push(format!("{}: the model is too thin to show up at this scale", path.display()));
    }
    Ok((shape, warnings))
}

// --------------------------------------------------------------------- OBJ --

fn load_obj(path: &Path, mats: &Materials, per_metre: f32) -> Result<(Shape, Vec<String>), String> {
    let (models, materials) = tobj::load_obj(path, &tobj::GPU_LOAD_OPTIONS).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut warnings = Vec::new();
    let materials = materials.unwrap_or_else(|e| {
        warnings.push(format!("{}: couldn't read its materials ({e})", path.display()));
        Vec::new()
    });
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut pictures = Vec::new();
    let mut unknown = Vec::new();
    // One paint per OBJ material.
    let paints: Vec<Paint> = materials
        .iter()
        .map(|m| {
            let tex = m.diffuse_texture.as_ref().and_then(|t| match Picture::open(&dir.join(t)) {
                Ok(p) => {
                    pictures.push(p);
                    Some(pictures.len() - 1)
                }
                Err(e) => {
                    warnings.push(format!("texture {t}: {e}"));
                    None
                }
            });
            let d = m.diffuse.unwrap_or([1.0, 1.0, 1.0]);
            // OBJ's PBR extension: `Pm` is how metallic it is.
            let metal = m.unknown_param.get("Pm").and_then(|v| v.trim().parse::<f32>().ok()).map(|pm| pm >= 0.5);
            choose_paint(&m.name, [d[0], d[1], d[2]], tex, metal, mats, &mut unknown)
        })
        .collect();
    let fallback = mats.id("stone").unwrap_or(AIR);
    let mut tris = Vec::new();
    for model in &models {
        let mesh = &model.mesh;
        let paint = mesh.material_id.and_then(|i| paints.get(i).cloned()).unwrap_or(Paint::Mat(fallback));
        let pos = |i: u32| -> [f32; 3] {
            let k = i as usize * 3;
            [mesh.positions[k], mesh.positions[k + 1], mesh.positions[k + 2]]
        };
        let uv = |i: u32| -> [f32; 2] {
            let k = i as usize * 2;
            if mesh.texcoords.len() >= k + 2 {
                // OBJ texture coordinates count up from the bottom.
                [mesh.texcoords[k], 1.0 - mesh.texcoords[k + 1]]
            } else {
                [0.0, 0.0]
            }
        };
        for t in mesh.indices.chunks_exact(3) {
            tris.push(Tri { p: [pos(t[0]), pos(t[1]), pos(t[2])], uv: [uv(t[0]), uv(t[1]), uv(t[2])], paint: paint.clone() });
        }
    }
    warnings.extend(unknown_warnings(path, unknown));
    let mut colors = ColorMap::new(path, mats, &mut warnings, true);
    let shape = rasterize(&tris, &pictures, &mut colors, per_metre, path)?;
    Ok((shape, warnings))
}

// ----------------------------------------------------------- via Blender --

/// Where Blender is: `GRAVE_BLENDER`, then `blender` on the PATH, then the
/// newest version in the usual install folder.
pub fn find_blender() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("GRAVE_BLENDER") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    if std::process::Command::new("blender").arg("--version").output().is_ok() {
        return Some(PathBuf::from("blender"));
    }
    let roots = [r"C:\Program Files\Blender Foundation", "/Applications/Blender.app/Contents/MacOS", "/usr/bin"];
    for root in roots {
        let Ok(rd) = std::fs::read_dir(root) else {
            let p = Path::new(root).join("blender");
            if p.exists() {
                return Some(p);
            }
            continue;
        };
        let mut found: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .map(|d| d.join(if cfg!(windows) { "blender.exe" } else { "blender" }))
            .filter(|p| p.exists())
            .collect();
        found.sort();
        if let Some(p) = found.pop() {
            return Some(p);
        }
    }
    None
}

/// Convert an .fbx or .blend file to .glb with Blender (in the background).
/// The result is cached until the original file changes.
fn via_blender(path: &Path) -> Result<PathBuf, String> {
    let blender = find_blender().ok_or(format!(
        "{}: importing this needs Blender installed (or export it from Blender as .glb). You can also set GRAVE_BLENDER to blender's path.",
        path.display()
    ))?;
    let src = std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let stamp = std::fs::metadata(&src).and_then(|m| m.modified()).map(|t| format!("{t:?}")).unwrap_or_default();
    let key = {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in src.to_string_lossy().bytes().chain(stamp.bytes()) {
            h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
        h
    };
    let out = std::env::temp_dir().join(format!("grave_import_{key:016x}.glb"));
    if out.exists() {
        return Ok(out);
    }
    let src_s = src.to_string_lossy().replace('\\', "/");
    let out_s = out.to_string_lossy().replace('\\', "/");
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mut cmd = std::process::Command::new(&blender);
    cmd.arg("-b").arg("--factory-startup");
    let script = if ext == "blend" {
        cmd.arg(&src);
        format!("import bpy\nbpy.ops.export_scene.gltf(filepath=r'{out_s}', export_format='GLB')\n")
    } else {
        format!(
            "import bpy\nbpy.ops.object.select_all(action='SELECT')\nbpy.ops.object.delete()\nbpy.ops.import_scene.fbx(filepath=r'{src_s}')\nbpy.ops.export_scene.gltf(filepath=r'{out_s}', export_format='GLB')\n"
        )
    };
    let result = cmd.arg("--python-expr").arg(script).output().map_err(|e| format!("couldn't run Blender: {e}"))?;
    if !out.exists() {
        let err = String::from_utf8_lossy(&result.stderr);
        let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no details");
        return Err(format!("{}: Blender couldn't convert it ({last})", path.display()));
    }
    Ok(out)
}

/// The Blender script that turns animated FBX files into a character.
const FBX_TO_CHARACTER: &str = include_str!("../tools/fbx_to_character.py");

/// Turn a folder of animated FBX files (e.g. downloaded from Mixamo: one per
/// animation, named Idle, Walking, Jumping, Falling, Swimming, Dying...) into
/// a character look in `out` (frames + character.txt), using Blender.
/// Returns Blender's report of what it made.
pub fn make_character(input: &Path, out: &Path) -> Result<String, String> {
    let blender = find_blender().ok_or("this needs Blender installed (or set GRAVE_BLENDER to blender.exe)")?;
    let script = std::env::temp_dir().join(format!("grave_fbx_to_character_{}.py", std::process::id()));
    std::fs::write(&script, FBX_TO_CHARACTER).map_err(|e| e.to_string())?;
    let result = std::process::Command::new(&blender)
        .arg("-b")
        .arg("--factory-startup")
        .arg("-P")
        .arg(&script)
        .arg("--")
        .arg(input)
        .arg(out)
        .output();
    let _ = std::fs::remove_file(&script);
    let output = result.map_err(|e| format!("couldn't run Blender: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr);
    let report: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("rendered ") || l.starts_with("skipping ") || l.starts_with("character written") || l.contains("Error") || l.starts_with("usage") || l.starts_with("no .fbx") || l.starts_with("none of"))
        .collect();
    if !out.join("character.txt").is_file() {
        let why = if report.is_empty() { text.trim().to_string() } else { report.join("
") };
        return Err(format!("Blender didn't make the character:
{why}"));
    }
    Ok(report.join("
"))
}
