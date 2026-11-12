//! Optional in-process timing for repeatable engine benchmarks. Disabled by default.
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
static ON: AtomicBool = AtomicBool::new(false);
static TIMES: Mutex<BTreeMap<&'static str, (u64, Duration, Duration)>> =
    Mutex::new(BTreeMap::new());
pub fn enable(on: bool) {
    ON.store(on, Ordering::Relaxed);
}
pub fn reset() {
    TIMES.lock().unwrap().clear();
}
pub struct Scope(Option<(&'static str, Instant)>);
#[inline]
pub fn scope(name: &'static str) -> Scope {
    Scope(ON.load(Ordering::Relaxed).then(|| (name, Instant::now())))
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some((name, t)) = self.0 {
            let d = t.elapsed();
            let mut times = TIMES.lock().unwrap();
            let entry = times.entry(name).or_default();
            entry.0 += 1;
            entry.1 += d;
            entry.2 = entry.2.max(d);
        }
    }
}
pub fn report() -> String {
    TIMES
        .lock()
        .unwrap()
        .iter()
        .map(|(name, (n, total, max))| {
            format!(
                "{name:25} calls {n:5} total {:8.1}ms mean {:7.3}ms peak {:7.2}ms\n",
                total.as_secs_f64() * 1000.,
                total.as_secs_f64() * 1000. / *n as f64,
                max.as_secs_f64() * 1000.
            )
        })
        .collect()
}
