//! Opt-in, dependency-free phase-boundary RSS trace for the `scan`
//! pipeline.
//!
//! Active only when `OPENDQI_MEM_TRACE` is set; otherwise every call
//! is a single env probe that returns immediately, so shipping
//! behaviour — and all golden / XSD-conformance output — is
//! byte-unchanged. This is a deliberate measurement aid (it answers
//! "which scan phase owns the peak RSS"), **not** wired into
//! `scripts/preflight.sh` or CI; drive it via
//! `scripts/bench-scale.sh --mem-trace`.
//!
//! The crate is `#![forbid(unsafe_code)]`, so RSS is read without FFI:
//! `/proc/self/statm` on Linux, a `ps` child on macOS.

/// Best-effort current resident-set size of this process, in bytes.
///
/// Coarse by design (page-granular; the macOS path spawns `ps`) —
/// adequate for GB-scale phase attribution, not byte precision.
/// Returns `None` on any failure; never panics, never affects flow.
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

/// Emit one stable trace line for `phase` iff `OPENDQI_MEM_TRACE` is
/// set; otherwise a no-op (single env probe, no allocation). The line
/// format is parsed by `scripts/bench-scale.sh --mem-trace`.
pub fn mem_trace(phase: &str) {
    if std::env::var_os("OPENDQI_MEM_TRACE").is_none() {
        return;
    }
    match current_rss_bytes() {
        Some(rss) => eprintln!("MEMTRACE phase={phase} rss_bytes={rss}"),
        None => eprintln!("MEMTRACE phase={phase} rss_bytes=unknown"),
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
}
