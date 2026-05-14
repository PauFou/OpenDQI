//! Shared server state: per-process temp directory + minijinja
//! template environment.

use std::path::PathBuf;

use anyhow::{Context, Result};
use minijinja::Environment;

use crate::templates;

/// Per-process state owned by the axum router.
pub struct AppState {
    /// Root directory under which scan artifacts are written.
    pub temp_root: PathBuf,
    /// Pre-compiled minijinja templates (index, results).
    pub templates: Environment<'static>,
}

impl AppState {
    /// Create a fresh process-scoped state. The temp root is rooted at
    /// `std::env::temp_dir()` with a PID + UUID suffix so concurrent
    /// `opendqi desktop` instances never clash.
    pub fn new() -> Result<Self> {
        let temp_root = std::env::temp_dir().join(format!(
            "opendqi-desktop-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_root)
            .with_context(|| format!("creating temp root {}", temp_root.display()))?;
        tracing::info!(?temp_root, "scan artifacts will be stored here");
        let templates = templates::build_env();
        Ok(Self {
            temp_root,
            templates,
        })
    }

    /// Resolve a path under the temp root, refusing any component that
    /// tries to escape the root.
    pub fn safe_join(&self, scan_id: &str, file: Option<&str>) -> Option<PathBuf> {
        if scan_id.contains(['/', '\\', '.']) || scan_id.is_empty() {
            return None;
        }
        let mut path = self.temp_root.join(scan_id);
        if let Some(file) = file {
            if file.contains(['/', '\\']) || file.contains("..") || file.is_empty() {
                return None;
            }
            path = path.join(file);
        }
        Some(path)
    }
}
