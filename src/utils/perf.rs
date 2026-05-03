use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfStatSnapshot {
    pub label: &'static str,
    pub count: u64,
    pub total: Duration,
    pub min: Duration,
    pub max: Duration,
}
#[derive(Debug, Default)]
struct PerfStat {
    count: u64,
    total: Duration,
    min: Option<Duration>,
    max: Duration,
}
impl PerfStat {
    fn record(&mut self, elapsed: Duration) {
        self.count = self.count.saturating_add(1);
        self.total = self.total.saturating_add(elapsed);
        self.min = Some(self.min.map_or(elapsed, |current| current.min(elapsed)));
        self.max = self.max.max(elapsed);
    }
    fn snapshot(&self, label: &'static str) -> PerfStatSnapshot {
        PerfStatSnapshot {
            label,
            count: self.count,
            total: self.total,
            min: self.min.unwrap_or_default(),
            max: self.max,
        }
    }
}
fn perf_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    // Cache MIRU_PERF once so instrumentation checks stay cheap inside render loops.
    *ENABLED.get_or_init(|| {
        std::env::var("MIRU_PERF")
            .ok()
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    })
}
fn perf_registry() -> &'static Mutex<BTreeMap<&'static str, PerfStat>> {
    static REGISTRY: OnceLock<Mutex<BTreeMap<&'static str, PerfStat>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()))
}
pub fn enabled() -> bool {
    perf_enabled()
}
pub fn reset() {
    if !enabled() {
        return;
    }
    perf_registry().lock().unwrap().clear();
}
pub fn record(label: &'static str, elapsed: Duration) {
    if !enabled() {
        return;
    }
    perf_registry()
        .lock()
        .unwrap()
        .entry(label)
        .or_default()
        .record(elapsed);
}
pub fn snapshot() -> Vec<PerfStatSnapshot> {
    if !enabled() {
        return Vec::new();
    }
    perf_registry()
        .lock()
        .unwrap()
        .iter()
        .map(|(label, stat)| stat.snapshot(label))
        .collect()
}
pub fn print_summary() {
    if !enabled() {
        return;
    }
    for stat in snapshot() {
        let avg = if stat.count == 0 {
            Duration::default()
        } else {
            Duration::from_secs_f64(stat.total.as_secs_f64() / stat.count as f64)
        };
        println!(
            "   [perf] {:<18} count={:<6} total={:>8.3}ms avg={:>8.3}ms min={:>8.3}ms max={:>8.3}ms",
            stat.label,
            stat.count,
            stat.total.as_secs_f64() * 1000.0,
            avg.as_secs_f64() * 1000.0,
            stat.min.as_secs_f64() * 1000.0,
            stat.max.as_secs_f64() * 1000.0,
        );
    }
}
pub struct PerfScope {
    label: &'static str,
    start: Option<Instant>,
}
impl PerfScope {
    pub fn disabled() -> Self {
        Self {
            label: "",
            start: None,
        }
    }
}
impl Drop for PerfScope {
    fn drop(&mut self) {
        let Some(start) = self.start.take() else {
            return;
        };
        // Scopes record on drop so early returns still contribute to timing totals.
        record(self.label, start.elapsed());
    }
}
pub fn scoped(label: &'static str) -> PerfScope {
    if !enabled() {
        return PerfScope::disabled();
    }
    PerfScope {
        label,
        start: Some(Instant::now()),
    }
}
