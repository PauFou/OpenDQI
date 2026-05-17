//! Opt-in, dependency-free RSS instrumentation for the `scan`
//! pipeline.
//!
//! Active only when `OPENDQI_MEM_TRACE` is set; otherwise every entry
//! point is a single env probe that returns immediately (no thread,
//! no allocation), so shipping behaviour — and all golden /
//! XSD-conformance output — is byte-unchanged. Deliberate measurement
//! aid, **not** wired into `scripts/preflight.sh` or CI; drive it via
//! `scripts/bench-scale.sh --mem-trace`.
//!
//! Two layers:
//! - [`mem_trace`] — *current* RSS at a named `run_scan` boundary
//!   (`MEMTRACE phase=<p> rss_bytes=<n>`); also records the live phase
//!   for the sampler.
//! - [`start_peak_sampler`] — a background thread polling RSS to catch
//!   the run *maximum* and attribute it to whichever phase was live,
//!   including a transient allocated-and-freed *between* boundaries
//!   (which boundary sampling structurally cannot see).
//!
//! The crate is `#![forbid(unsafe_code)]`, so RSS is read without FFI
//! (`/proc/self/statm` on Linux, a `ps` child on macOS) and the
//! sampler uses only safe `std` threading/atomics.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Phase markers in pipeline order. The sampler stores an index into
/// this table; unknown names map to index 0 (`start`).
const PHASES: [&str; 11] = [
    "start",
    "post_discovery",
    "post_parse",
    "post_checks",
    "post_lifecycle_presub",
    "post_finalize",
    "post_summary",
    "post_write_summary_json",
    "post_write_issues_csv",
    "post_write_report_html",
    "post_report",
];

fn phase_id(name: &str) -> u8 {
    PHASES.iter().position(|&p| p == name).unwrap_or(0) as u8
}

fn phase_name(id: u8) -> &'static str {
    PHASES.get(id as usize).copied().unwrap_or("unknown")
}

/// Last phase marker reached, read by the sampler thread to attribute
/// the peak. `Relaxed` is sufficient — this is a coarse diagnostic,
/// not a synchronisation primitive.
static CURRENT_PHASE: AtomicU8 = AtomicU8::new(0);

/// Best-effort current resident-set size of this process, in bytes.
///
/// Coarse by design (page-granular; the macOS path spawns `ps`) —
/// adequate for GB-scale attribution, not byte precision. Returns
/// `None` on any failure; never panics, never affects flow.
pub fn current_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        // `/proc/self/statm` fields are page counts; the 2nd (index 1)
        // is "resident". Assume the standard 4 KiB page — coarse but
        // sufficient for GB buckets.
        let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
        let resident_pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
        Some(resident_pages * 4096)
    }
    #[cfg(not(target_os = "linux"))]
    {
        // macOS (the dev box) has no `/proc`; `ps -o rss=` reports the
        // target process RSS in KiB. std-only, no FFI.
        let pid = std::process::id().to_string();
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let kib: u64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
        Some(kib * 1024)
    }
}

/// Emit one stable trace line for `phase` and record it as the live
/// phase for the sampler, iff `OPENDQI_MEM_TRACE` is set; otherwise a
/// no-op (single env probe, no allocation, no atomic store). The line
/// format is parsed by `scripts/bench-scale.sh --mem-trace`.
pub fn mem_trace(phase: &str) {
    if std::env::var_os("OPENDQI_MEM_TRACE").is_none() {
        return;
    }
    CURRENT_PHASE.store(phase_id(phase), Ordering::Relaxed);
    match current_rss_bytes() {
        Some(rss) => eprintln!("MEMTRACE phase={phase} rss_bytes={rss}"),
        None => eprintln!("MEMTRACE phase={phase} rss_bytes=unknown"),
    }
}

fn sampler_interval() -> Duration {
    let ms = std::env::var("OPENDQI_MEM_TRACE_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(200)
        .max(10); // never busy-spin
    Duration::from_millis(ms)
}

/// RAII handle for the background peak sampler. Held in a **named**
/// binding for the lifetime of `run_scan`; on drop it stops and joins
/// the thread (which prints the `MEMTRACE peak …` line).
pub struct PeakSampler {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
    enabled: bool,
}

/// Start the peak sampler iff `OPENDQI_MEM_TRACE` is set. When unset,
/// returns a disabled handle: no thread spawned, `Drop` is a no-op.
pub fn start_peak_sampler() -> PeakSampler {
    if std::env::var_os("OPENDQI_MEM_TRACE").is_none() {
        return PeakSampler {
            stop: Arc::new(AtomicBool::new(true)),
            handle: None,
            enabled: false,
        };
    }
    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = Arc::clone(&stop);
    let interval = sampler_interval();
    let started = Instant::now();
    let handle = std::thread::Builder::new()
        .name("opendqi-memtrace".into())
        .spawn(move || {
            let mut peak: u64 = 0;
            let mut at_ms: u128 = 0;
            let mut at_phase: u8 = 0;
            let mut observe = || {
                if let Some(r) = current_rss_bytes() {
                    if r > peak {
                        peak = r;
                        at_ms = started.elapsed().as_millis();
                        at_phase = CURRENT_PHASE.load(Ordering::Relaxed);
                    }
                }
            };
            while !stop_t.load(Ordering::Relaxed) {
                observe();
                std::thread::sleep(interval);
            }
            observe(); // final read so a late spike isn't missed
            eprintln!(
                "MEMTRACE peak rss_bytes={peak} at_ms={at_ms} phase={}",
                phase_name(at_phase)
            );
        })
        .ok();
    PeakSampler {
        stop,
        handle,
        enabled: true,
    }
}

impl Drop for PeakSampler {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_rss_is_some_and_positive() {
        let rss = current_rss_bytes().expect("RSS readable on the dev platform");
        assert!(rss > 0, "RSS must be positive, got {rss}");
    }

    #[test]
    fn mem_trace_does_not_panic() {
        // Behaviour depends on the ambient `OPENDQI_MEM_TRACE`; either
        // branch must be caller-side-effect-free and must not panic.
        // (No env mutation — `std::env::set_var`/`remove_var` are
        // `unsafe` under edition 2024 and this crate forbids unsafe;
        // also the suite runs in parallel.)
        mem_trace("unit_test_phase");
    }

    #[test]
    fn disabled_sampler_spawns_nothing_and_drops_cleanly() {
        // The test suite runs without `OPENDQI_MEM_TRACE`; the handle
        // must be inert (no thread) and its drop must not panic.
        let s = start_peak_sampler();
        assert!(!s.enabled, "sampler must be disabled without the env");
        assert!(s.handle.is_none(), "no thread may be spawned");
        drop(s); // no-op Drop must not panic
    }

    #[test]
    fn phase_id_round_trips_known_names() {
        for (i, name) in PHASES.iter().enumerate() {
            assert_eq!(phase_id(name) as usize, i);
            assert_eq!(phase_name(i as u8), *name);
        }
        assert_eq!(phase_id("nope"), 0);
        assert_eq!(phase_name(250), "unknown");
    }
}
