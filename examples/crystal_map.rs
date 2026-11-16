//! Lists crystal patches in a scene and their distance from a point (for level design).
use grave_engine::{Materials, scene};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (file, hx, hy) = (&a[1], a[2].parse::<i32>().unwrap(), a[3].parse::<i32>().unwrap());
    let m = Materials::load_dir("materials");
    let (s, _) = scene::load(&std::fs::read_to_string(file).unwrap(), &m, Some(std::path::Path::new("scenes"))).into_sim(&m);
    let w = &s.world;
    let ids = [m.id("crystal_cyan").unwrap(), m.id("crystal_violet").unwrap()];
    let mut seen = vec![false; w.mat.len()];
    for i in 0..w.mat.len() {
        let k = match ids.iter().position(|&c| c == w.mat[i]) { Some(k) => k, None => continue };
        if seen[i] { continue; }
        let (mut n, mut sx, mut sy, mut st) = (0, 0i64, 0i64, vec![i]);
        seen[i] = true;
        while let Some(j) = st.pop() {
            n += 1; let (x, y) = ((j % w.w) as i64, (j / w.w) as i64); sx += x; sy += y;
            for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w.w as i64 || ny >= w.h as i64 { continue; }
                let q = (ny as usize) * w.w + nx as usize;
                if !seen[q] && w.mat[q] == ids[k] { seen[q] = true; st.push(q); }
            }
        }
        let (cx, cy) = ((sx / n) as i32, (sy / n) as i32);
        let d = (((cx - hx).pow(2) + (cy - hy).pow(2)) as f32).sqrt();
        println!("{} {:>4} {:>4} cells {:>3} dist {:>5.0}", ["cyan  ", "violet"][k], cx, cy, n, d);
    }
}
