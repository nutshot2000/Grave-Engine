//! Broken scene files never crash the loader or the first frames: every
//! scene, with random words swapped for nonsense, loads (with problems) and runs.
use grave_engine::{Materials, scene};

fn scenes() -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/scenes"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt") && !p.to_string_lossy().contains(".local."))
        .collect();
    v.sort();
    v
}

fn rng(seed: &mut u64) -> u64 { *seed ^= *seed << 13; *seed ^= *seed >> 7; *seed ^= *seed << 17; *seed }

#[test]
fn broken_scenes_never_crash_the_loader() {
    let m = Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"));
    let weird = ["-1", "0", "999999", "-999999", "NaN", "inf", "-inf", "1e30", "0.0001", "", "x", "#zz", "ground", "air", "3.5"];
    let mut seed = 0x1234_5678_9abc_def0u64;
    let mut crashes = Vec::new();
    for p in scenes() {
        let text = std::fs::read_to_string(&p).unwrap();
        let lines: Vec<&str> = text.lines().filter(|l| !l.starts_with("paint ")).take(200).collect();
        for _round in 0..4 {
            let mut out = Vec::new();
            for l in &lines {
                let mut w: Vec<String> = l.split_whitespace().map(String::from).collect();
                if !w.is_empty() && rng(&mut seed) % 4 == 0 {
                    let k = (rng(&mut seed) as usize) % w.len();
                    w[k] = weird[(rng(&mut seed) as usize) % weird.len()].to_string();
                }
                out.push(w.join(" "));
            }
            let t = out.join("\n");
            let mm = &m;
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let (mut s, _) = scene::load(&t, mm, p.parent()).into_sim(mm);
                for _ in 0..3 { s.step(mm); }
            }));
            if r.is_err() {
                crashes.push(p.file_name().unwrap().to_string_lossy().to_string());
                std::fs::write(format!("{}/shots/fuzz-crash-{}.txt", env!("CARGO_MANIFEST_DIR"), crashes.len()), &t).ok();
                break;
            }
        }
    }
    assert!(crashes.is_empty(), "a broken scene crashed the loader (saved in shots/fuzz-crash-*.txt): {crashes:?}");
}
