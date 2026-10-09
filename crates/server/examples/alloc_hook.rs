//! Sampling allocation profiler, compiled into `bot_cpu` via `#[path]`.
//!
//! Enabled with `BGD_ALLOC_PROFILE=1` (optional `BGD_ALLOC_SAMPLE=N`, default
//! 256 -- one backtrace per N allocations). Totals count every allocation; the
//! backtrace aggregate is scaled by the sample rate at dump time.
//!
//! Dump: `alloc_hook::dump()` at process exit, or any time from `main`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

static ENABLED: AtomicBool = AtomicBool::new(false);
static SAMPLE_N: AtomicU64 = AtomicU64::new(256);
static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static FREE_COUNT: AtomicU64 = AtomicU64::new(0);
static SAMPLED: AtomicU64 = AtomicU64::new(0);

struct Site {
    count: u64,
    bytes: u64,
}

// `HashMap::new` is not const -- lazy-lock the aggregates instead.
static SITES: std::sync::LazyLock<Mutex<HashMap<u64, Site>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
static TRACES: std::sync::LazyLock<Mutex<HashMap<u64, String>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct ProfAlloc;

fn sampling_on() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

unsafe impl GlobalAlloc for ProfAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if sampling_on() {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            let n = SAMPLE_N.load(Ordering::Relaxed).max(1);
            if ALLOC_COUNT.load(Ordering::Relaxed) % n == 0 {
                SAMPLED.fetch_add(1, Ordering::Relaxed);
                record(layout.size());
            }
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if sampling_on() {
            FREE_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if sampling_on() {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
            let n = SAMPLE_N.load(Ordering::Relaxed).max(1);
            if ALLOC_COUNT.load(Ordering::Relaxed) % n == 0 {
                SAMPLED.fetch_add(1, Ordering::Relaxed);
                record(new_size);
            }
        }
        System.realloc(ptr, layout, new_size)
    }
}

fn record(size: usize) {
    // Re-entrancy guard: `Backtrace::force_capture` and the map locks allocate.
    thread_local! {
        static IN_RECORD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    let already = IN_RECORD.with(|f| f.replace(true));
    if already {
        return;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let bt = std::backtrace::Backtrace::force_capture();
        let s = format!("{bt}");
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in s.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        {
            let mut sites = SITES.lock().unwrap();
            let e = sites.entry(h).or_insert(Site { count: 0, bytes: 0 });
            e.count += 1;
            e.bytes += size as u64;
        }
        let mut traces = TRACES.lock().unwrap();
        traces.entry(h).or_insert(s);
    }));
    let _ = result;
    IN_RECORD.with(|f| f.set(false));
}

/// Called once at start of `main`. Reads `BGD_ALLOC_PROFILE` / `BGD_ALLOC_SAMPLE`.
pub fn init() {
    if std::env::var_os("BGD_ALLOC_PROFILE").is_some_and(|v| v != "0" && v != "") {
        if let Ok(n) = std::env::var("BGD_ALLOC_SAMPLE") {
            if let Ok(n) = n.parse::<u64>() {
                SAMPLE_N.store(n.max(1), Ordering::Relaxed);
            }
        }
        ENABLED.store(true, Ordering::SeqCst);
        eprintln!(
            "alloc_hook: on (sample 1/{})",
            SAMPLE_N.load(Ordering::Relaxed)
        );
    }
}

/// Pretty-print the aggregate. Scaled counts/bytes by the sample rate.
pub fn dump() {
    if !sampling_on() {
        return;
    }
    let n = SAMPLE_N.load(Ordering::Relaxed).max(1) as f64;
    let total_c = ALLOC_COUNT.load(Ordering::Relaxed);
    let total_b = ALLOC_BYTES.load(Ordering::Relaxed);
    let frees = FREE_COUNT.load(Ordering::Relaxed);
    let sampled = SAMPLED.load(Ordering::Relaxed);
    eprintln!("\n=== allocation profile (sampling 1/{n}) ===");
    eprintln!(
        "allocs {} ({:.1} M), bytes {} ({:.1} GiB), frees {}, sampled {}",
        total_c,
        total_c as f64 / 1e6,
        total_b,
        total_b as f64 / (1024.0 * 1024.0 * 1024.0),
        frees,
        sampled
    );
    let sites = SITES.lock().unwrap();
    let traces = TRACES.lock().unwrap();
    let mut rows: Vec<(u64, u64, u64, &str)> = Vec::new();
    for (h, s) in sites.iter() {
        let t = traces.get(h).map(|s| s.as_str()).unwrap_or("?");
        rows.push((s.bytes, s.count, *h, t));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    eprintln!("-- top 30 sites by sampled bytes (scaled) --");
    for (i, (bytes, count, _h, trace)) in rows.iter().take(30).enumerate() {
        // First 6 interesting frames (skip the backtrace header / alloc internals).
        let frames: Vec<&str> = trace
            .lines()
            .filter(|l| l.contains(" at ") || l.contains("!"))
            .filter(|l| {
                let l = l.to_ascii_lowercase();
                !(l.contains("alloc_hook")
                    || l.contains("backtrace")
                    || l.contains("force_capture")
                    || l.contains("record"))
            })
            .take(8)
            .collect();
        let head = frames
            .first()
            .copied()
            .unwrap_or(trace.lines().next().unwrap_or("?"));
        eprintln!(
            "  {:>2}. scaled {:>7.1} MiB  {:>8.0} allocs  {}",
            i + 1,
            *bytes as f64 * n / (1024.0 * 1024.0),
            *count as f64 * n,
            head.trim()
        );
        for f in frames.iter().skip(1).take(3) {
            eprintln!("        {}", f.trim());
        }
    }
}