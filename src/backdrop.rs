//! Backdrops: a sky and layers of scenery behind the world, for depth.
//!
//! ```text
//! sky #1b2a4a #d07050 stars              # gradient from the top colour to the bottom one; stars at night
//! layer mountains #40486a depth 0.15 y 150 height 90 snow
//! layer hills #2e3d4a depth 0.35 y 190 height 40 seed 3
//! layer city #1f2430 depth 0.5 y 200 height 70 windows
//! layer forest #1c2a22 depth 0.7 y 205 height 25
//! layer clouds #ffffff depth 0.1 y 40 height 20
//! layer image assets/backdrops/hills.png depth 0.3 y 200 scale 2
//! ```
//!
//! - Layers are listed far to near. `depth` says how far away: 0 = as far
//!   as the sky (doesn't move), 1 = as near as the world (moves with it).
//!   Far layers slide past slowly as the camera moves: parallax.
//! - `y` is where the layer's ground line is (world cells) when the camera
//!   is in the middle of the world, `height` how tall its shapes get.
//! - Kinds: `hills` (rolling), `mountains` (jagged; `snow` caps), `city`
//!   (skyline; `windows` that light up in the dark), `forest` (pines),
//!   `clouds` (drift slowly), `image FILE` (a picture tiled sideways; `scale`).
//! - `seed N` changes the shapes; `haze 0..1` fades a layer into the sky.
//! - The scene's lighting dims them (night, dusk); windows glow on their own.
//! - Only empty air shows the backdrop. No `sky` or `layer` lines = the
//!   plain background, as before.

use crate::import::Picture;

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Hills,
    Mountains,
    City,
    Forest,
    Clouds,
    Image(std::path::PathBuf),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub kind: Kind,
    pub color: [u8; 3],
    pub depth: f32,
    pub y: f32,
    pub height: f32,
    pub seed: u32,
    /// Snow caps (mountains) / lit windows (city).
    pub extra: bool,
    /// Fade towards the sky colour, 0..1.
    pub haze: f32,
    /// Image scale (cells per picture pixel).
    pub scale: f32,
}

impl Layer {
    pub fn new(kind: Kind, color: [u8; 3], depth: f32, y: f32, height: f32) -> Layer {
        Layer { kind, color, depth: depth.clamp(0.0, 1.0), y, height: height.max(1.0), seed: 1, extra: false, haze: 0.0, scale: 1.0 }
    }

    fn kind_word(&self) -> String {
        match &self.kind {
            Kind::Hills => "hills".into(),
            Kind::Mountains => "mountains".into(),
            Kind::City => "city".into(),
            Kind::Forest => "forest".into(),
            Kind::Clouds => "clouds".into(),
            Kind::Image(p) => {
                let name = p.display().to_string();
                if name.contains(char::is_whitespace) { format!("image \"{name}\"") } else { format!("image {name}") }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Backdrop {
    /// Sky gradient (top, bottom) and stars. None = the plain background.
    pub sky: Option<([u8; 3], [u8; 3], bool)>,
    pub layers: Vec<Layer>,
    /// The scene's folder: pictures named relative to it are found there
    /// (the names are kept as written, so saved scenes stay portable).
    pub dir: Option<std::path::PathBuf>,
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn hash(a: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA77);
    x ^= x >> 13;
    x
}

fn rand(a: u32) -> f32 {
    hash(a) as f32 / u32::MAX as f32
}

/// Smooth 1D noise, 0..1.
fn noise(x: f32, seed: u32) -> f32 {
    let i = x.floor();
    let f = x - i;
    let a = rand((i as i32 as u32).wrapping_mul(374761393) ^ seed);
    let b = rand(((i as i32 + 1) as u32).wrapping_mul(374761393) ^ seed);
    let t = f * f * (3.0 - 2.0 * f);
    a + (b - a) * t
}

/// Ready-made backdrops: (name, scene lines).
pub const PRESETS: &[(&str, &str)] = &[
    ("☀ Day hills", "sky #5b8fd0 #bfe0f5\nlayer clouds #ffffff depth 0.05 y 25 height 22\nlayer hills #7a9ab0 depth 0.15 y 150 height 50 seed 4 haze 0.3\nlayer hills #5f8a5a depth 0.35 y 175 height 40 seed 2\nlayer forest #3d6a40 depth 0.6 y 190 height 25"),
    ("🌇 Sunset mountains", "sky #2c3e6a #f09a5a\nlayer clouds #f4c0a0 depth 0.05 y 30 height 20\nlayer mountains #5a5a86 depth 0.12 y 160 height 100 snow seed 2\nlayer hills #3e4660 depth 0.3 y 180 height 45 seed 5\nlayer forest #262c38 depth 0.6 y 195 height 22"),
    ("🌃 Night city", "sky #0b1022 #2a3458 stars\nlayer mountains #1e2440 depth 0.1 y 150 height 80 seed 9\nlayer city #161a26 depth 0.35 y 185 height 80 windows seed 3\nlayer city #10131c depth 0.6 y 195 height 50 windows seed 8"),
    ("🏜 Desert", "sky #4f86c6 #f2d6a2\nlayer mountains #c09a78 depth 0.1 y 160 height 60 seed 6 haze 0.35\nlayer hills #d9b480 depth 0.3 y 185 height 35 seed 7\nlayer hills #c49a66 depth 0.55 y 195 height 25 seed 11"),
    ("🌲 Forest", "sky #6c9cc8 #d8ecf0\nlayer mountains #7d92a8 depth 0.1 y 150 height 80 seed 3 haze 0.4\nlayer forest #4a6e58 depth 0.3 y 175 height 45 seed 2 haze 0.2\nlayer forest #2f5038 depth 0.55 y 190 height 40 seed 5\nlayer forest #1d3424 depth 0.8 y 200 height 30 seed 9"),
];

impl Backdrop {
    /// A ready-made backdrop, fitted to a world `h` cells tall (the presets
    /// are drawn for 220).
    pub fn preset(lines: &str, h: usize) -> Backdrop {
        let mut b = Backdrop::default();
        for line in lines.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            let _ = b.parse_line(&words);
        }
        let k = h as f32 / 220.0;
        for l in b.layers.iter_mut() {
            l.y *= k;
            l.height *= k;
        }
        b
    }

    pub fn is_empty(&self) -> bool {
        self.sky.is_none() && self.layers.is_empty()
    }

    /// Read one scene line (`sky ...` or `layer ...`). Ok(false) = not ours.
    pub fn parse_line(&mut self, words: &[&str]) -> Result<bool, String> {
        // A picture name with spaces goes in quotes: `layer image "my hills.png" ...`.
        let joined: Vec<String>;
        let words: Vec<&str> = if words.get(2).is_some_and(|w| w.starts_with('"')) {
            let end = (2..words.len()).find(|&k| words[k].ends_with('"') && (k > 2 || words[k].len() > 1)).ok_or("a quoted picture name needs its closing quote")?;
            let name = words[2..=end].join(" ");
            joined = std::iter::once(words[0].to_string())
                .chain(std::iter::once(words[1].to_string()))
                .chain(std::iter::once(name.trim_matches('"').to_string()))
                .chain(words[end + 1..].iter().map(|w| w.to_string()))
                .collect();
            joined.iter().map(String::as_str).collect()
        } else {
            words.to_vec()
        };
        let words = &words[..];
        let color = |w: Option<&&str>| -> Result<[u8; 3], String> { crate::material::parse_color(w.ok_or("missing a colour like #2e3d4a")?) };
        match words.first().map(|w| w.to_lowercase()).as_deref() {
            Some("sky") => {
                if words.get(1).is_some_and(|w| w.eq_ignore_ascii_case("off")) {
                    self.sky = None;
                    return Ok(true);
                }
                let top = color(words.get(1)).map_err(|_| "use `sky #top #bottom [stars]`".to_string())?;
                let bottom = words.get(2).filter(|w| w.starts_with('#')).map(|w| crate::material::parse_color(w)).transpose()?.unwrap_or(top);
                let stars = words.iter().any(|w| w.eq_ignore_ascii_case("stars"));
                self.sky = Some((top, bottom, stars));
                Ok(true)
            }
            Some("layer") => {
                let usage = "use `layer hills|mountains|city|forest|clouds #colour depth D y Y height H` or `layer image FILE depth D y Y`";
                let kind_word = words.get(1).ok_or(usage)?.to_lowercase();
                let (kind, mut i) = match kind_word.as_str() {
                    "hills" => (Kind::Hills, 2),
                    "mountains" => (Kind::Mountains, 2),
                    "city" => (Kind::City, 2),
                    "forest" | "trees" => (Kind::Forest, 2),
                    "clouds" => (Kind::Clouds, 2),
                    "image" | "picture" => (Kind::Image(std::path::PathBuf::from(words.get(2).ok_or(usage)?)), 3),
                    other => return Err(format!("layer: don't know `{other}` ({usage})")),
                };
                let mut c = [128, 128, 128];
                if let Some(w) = words.get(i).filter(|w| w.starts_with('#')) {
                    c = crate::material::parse_color(w)?;
                    i += 1;
                }
                let mut l = Layer::new(kind, c, 0.5, 200.0, 40.0);
                while i < words.len() {
                    let num = |k: usize| words.get(k).and_then(|w| w.parse::<f32>().ok()).ok_or(format!("layer: `{}` needs a number", words[k - 1]));
                    match words[i].to_lowercase().as_str() {
                        "depth" => { l.depth = num(i + 1)?.clamp(0.0, 1.0); i += 2; }
                        "y" => { l.y = num(i + 1)?; i += 2; }
                        "height" => { l.height = num(i + 1)?.max(1.0); i += 2; }
                        "seed" => { l.seed = num(i + 1)? as u32; i += 2; }
                        "haze" => { l.haze = num(i + 1)?.clamp(0.0, 1.0); i += 2; }
                        "scale" => { l.scale = num(i + 1)?.clamp(0.1, 20.0); i += 2; }
                        "snow" | "windows" => { l.extra = true; i += 1; }
                        other => return Err(format!("layer: don't know `{other}`")),
                    }
                }
                self.layers.push(l);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// The scene lines.
    pub fn to_lines(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut s = String::from("\n# Backdrop (far to near)\n");
        if let Some((t, b, stars)) = self.sky {
            s += &format!("sky {} {}{}\n", hex(t), hex(b), if stars { " stars" } else { "" });
        }
        for l in &self.layers {
            let colour = if matches!(l.kind, Kind::Image(_)) { String::new() } else { format!(" {}", hex(l.color)) };
            s += &format!("layer {}{colour} depth {} y {} height {} seed {}", l.kind_word(), l.depth, l.y, l.height, l.seed);
            if l.haze > 0.0 {
                s += &format!(" haze {}", l.haze);
            }
            if l.scale != 1.0 {
                s += &format!(" scale {}", l.scale);
            }
            if l.extra {
                s += match l.kind { Kind::Mountains => " snow", _ => " windows" };
            }
            s.push('\n');
        }
        s
    }

    /// Draw the backdrop at world resolution for a camera centred at
    /// (cx, cy) (cells): `out[i]` = colour, `glow[i]` = light it gives off
    /// itself (lit windows, stars), for every cell. `frame` drifts clouds.
    ///
    /// Only the window `win` = (x, y, w, h) of the world is drawn: `out` and
    /// `glow` are `w * h`, row by row.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, w: usize, h: usize, win: (usize, usize, usize, usize), cx: f32, cy: f32, frame: u32, pictures: &mut std::collections::HashMap<std::path::PathBuf, Option<Picture>>, out: &mut Vec<[f32; 3]>, glow: &mut Vec<[f32; 3]>) {
        let (wx0, wy0, ww, wh) = win;
        let n = ww * wh;
        let (wx1, wy1) = (wx0 + ww, wy0 + wh);
        let local = |x: usize, y: usize| (y - wy0) * ww + (x - wx0);
        out.clear();
        glow.clear();
        glow.resize(n, [0.0; 3]);
        let (top, bottom, stars) = self.sky.unwrap_or(([0, 0, 0], [0, 0, 0], false));
        let f3 = |c: [u8; 3]| [c[0] as f32, c[1] as f32, c[2] as f32];
        out.reserve(n);
        for y in wy0..wy1 {
            let t = y as f32 / (h.max(2) - 1) as f32;
            let (a, b) = (f3(top), f3(bottom));
            let c = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
            for _ in 0..ww {
                out.push(c);
            }
        }
        if stars {
            // Stars stay put (they're infinitely far) and twinkle a little.
            for j in 0..n {
                let (x, y) = (wx0 + j % ww, wy0 + j / ww);
                let r = hash((y * w + x) as u32 ^ 0x5bd1e995);
                if r % 173 == 0 && y < h * 2 / 3 {
                    let tw = 0.6 + 0.4 * rand(r ^ (frame / 20));
                    let v = 150.0 * tw;
                    glow[j] = [v, v, v * 1.1];
                }
            }
        }
        let sky_at = |y: usize| -> [f32; 3] {
            let t = y as f32 / (h.max(2) - 1) as f32;
            let (a, b) = (f3(top), f3(bottom));
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
        };
        let y_mid = h as f32 / 2.0;
        let mut tops = vec![0.0f32; ww];
        for l in &self.layers {
            let shift_x = cx * (1.0 - l.depth);
            let shift_y = (cy - y_mid) * (1.0 - l.depth);
            let col = f3(l.color);
            let seed = hash(l.seed.wrapping_mul(2654435761));
            match &l.kind {
                Kind::Image(path) => {
                    // As written (from the project folder), or next to the scene.
                    let found = match &self.dir {
                        Some(d) if path.is_relative() && !path.exists() => d.join(path),
                        _ => path.clone(),
                    };
                    let pic = pictures.entry(found.clone()).or_insert_with(|| Picture::open(&found).ok()).clone();
                    let Some(p) = pic else { continue };
                    let s = l.scale.max(0.1);
                    let (pw, ph) = (p.w as f32 * s, p.h as f32 * s);
                    for y in wy0..wy1 {
                        let ly = y as f32 - shift_y - (l.y - ph); // from the picture's top
                        if ly < 0.0 || ly >= ph {
                            continue;
                        }
                        for x in wx0..wx1 {
                            let lx = (x as f32 - shift_x).rem_euclid(pw);
                            let (c, a) = p.sample(lx / pw, ly / ph);
                            if a < 128 {
                                continue;
                            }
                            let i = local(x, y);
                            let k = l.haze;
                            let sky = sky_at(y);
                            out[i] = [c[0] as f32 * (1.0 - k) + sky[0] * k, c[1] as f32 * (1.0 - k) + sky[1] * k, c[2] as f32 * (1.0 - k) + sky[2] * k];
                            glow[i] = [0.0; 3];
                        }
                    }
                    continue;
                }
                Kind::Clouds => {
                    // Puffy clouds: each column has a cloud this thick,
                    // centred on the band, with soft edges.
                    let drift = frame as f32 * 0.02;
                    let mid = l.y + l.height / 2.0 + shift_y;
                    for x in wx0..wx1 {
                        let lx = x as f32 - shift_x + drift;
                        let d = noise(lx / 30.0, seed) * 0.65 + noise(lx / 9.0, seed ^ 99) * 0.35;
                        let thick = ((d - 0.45) * 2.2).clamp(0.0, 1.0) * l.height / 2.0;
                        if thick <= 0.5 {
                            continue;
                        }
                        // Flatter bottoms, rounder tops.
                        let (y0, y1) = ((mid - thick).floor().max(wy0 as f32) as usize, (mid + thick * 0.45).ceil().min(wy1 as f32).max(0.0) as usize);
                        for y in y0..y1 {
                            let dy = (y as f32 + 0.5 - mid) / if (y as f32) < mid { thick } else { thick * 0.45 };
                            let a = ((1.0 - dy.abs()) * 3.0).clamp(0.0, 1.0) * 0.85 * (1.0 - l.haze);
                            let shade = 1.0 - (dy.max(0.0)) * 0.25; // a little darker underneath
                            let i = local(x, y);
                            for k in 0..3 {
                                out[i][k] = out[i][k] * (1.0 - a) + col[k] * shade * a;
                                glow[i][k] *= 1.0 - a; // clouds hide the stars
                            }
                        }
                    }
                    continue;
                }
                _ => {}
            }
            // Solid silhouettes: the top of the shape in each column.
            for (x, t) in tops.iter_mut().enumerate() {
                let lx = (wx0 + x) as f32 - shift_x;
                let height = match l.kind {
                    Kind::Hills => noise(lx / 70.0, seed) * 0.7 + noise(lx / 23.0, seed ^ 7) * 0.3,
                    Kind::Mountains => {
                        let r = |s: f32, k: u32| 1.0 - (noise(lx / s, seed ^ k) * 2.0 - 1.0).abs();
                        (r(60.0, 1) * 0.6 + r(19.0, 2) * 0.3 + r(6.0, 3) * 0.1).powf(1.6)
                    }
                    Kind::City => {
                        // Buildings 6-18 wide, random heights.
                        let mut start = (lx / 12.0).floor() * 12.0;
                        let b = (start / 12.0) as i32 as u32;
                        let wdt = 6.0 + rand(b ^ seed) * 10.0;
                        if lx - start > wdt + 1.0 {
                            start = f32::NAN; // a gap between buildings: low
                        }
                        if start.is_nan() { 0.08 } else { 0.25 + rand(b.wrapping_mul(31) ^ seed) * 0.75 }
                    }
                    Kind::Forest => {
                        let cell = (lx / 5.0).floor();
                        let b = cell as i32 as u32;
                        let peak = 0.45 + rand(b ^ seed) * 0.55;
                        let dx = (lx - (cell * 5.0 + 2.5)).abs() / 2.5; // 0 at the trunk
                        let base = noise(lx / 40.0, seed ^ 5) * 0.3 + 0.25;
                        (peak * (1.0 - dx)).max(base)
                    }
                    _ => 0.0,
                };
                *t = l.y - height * l.height + shift_y;
            }
            for x in 0..ww {
                let t0 = tops[x].max(wy0 as f32);
                let y0 = t0.ceil() as usize;
                if y0 >= wy1 {
                    continue;
                }
                let lx = (wx0 + x) as f32 - shift_x;
                for y in y0..wy1 {
                    let i = (y - wy0) * ww + x;
                    let below = y as f32 - tops[x];
                    // A little darker further down; haze blends into the sky.
                    let shade = 1.0 - (below / (l.height * 2.0)).min(1.0) * 0.25;
                    let mut c = [col[0] * shade, col[1] * shade, col[2] * shade];
                    let mut g = [0.0f32; 3];
                    match l.kind {
                        Kind::Mountains if l.extra && below < l.height * 0.12 && tops[x] < l.y - l.height * 0.55 => c = [235.0, 240.0, 250.0],
                        Kind::City if l.extra => {
                            let (wx, wy) = (lx.rem_euclid(3.0), (y as f32 - shift_y).rem_euclid(4.0));
                            if wx < 1.0 && wy < 2.0 && below > 2.0 {
                                let r = rand((lx.floor() as i32 as u32).wrapping_mul(7) ^ ((y as f32 - shift_y).floor() as i32 as u32).wrapping_mul(131) ^ seed);
                                if r < 0.35 {
                                    c = [255.0, 200.0, 120.0];
                                    g = [180.0, 130.0, 60.0];
                                }
                            }
                        }
                        _ => {}
                    }
                    if l.haze > 0.0 {
                        let sky = sky_at(y);
                        for k in 0..3 {
                            c[k] = c[k] * (1.0 - l.haze) + sky[k] * l.haze;
                        }
                    }
                    out[i] = c;
                    glow[i] = g;
                }
            }
        }
    }
}
