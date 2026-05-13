//! Input discovery: turn a path (file or directory) into a list of input files.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

/// Extensions accepted by `discover_emir_inputs`.
const SUPPORTED_EXTENSIONS: &[&str] = &["csv", "xml"];

/// Discover EMIR inputs at `path`.
///
/// - A single file is returned as-is when its extension is one of
///   `csv` / `xml` (case-insensitive). Other single-file extensions
///   are still returned (the caller decides what to do); archives
///   (`zip`, `gz`) are rejected with a clear error.
/// - A directory is scanned non-recursively for `*.csv` and `*.xml`
///   files, sorted by name for deterministic output.
pub fn discover_emir_inputs(path: &Path) -> Result<Vec<PathBuf>> {
    let meta = std::fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    if meta.is_file() {
        if has_extension(path, "zip") || has_extension(path, "gz") {
            return Err(anyhow!(
                "archives are not yet supported; pass an unzipped CSV or XML file"
            ));
        }
        return Ok(vec![path.to_path_buf()]);
    }
    if meta.is_dir() {
        let mut out: Vec<PathBuf> = walkdir::WalkDir::new(path)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .filter(|p| SUPPORTED_EXTENSIONS.iter().any(|ext| has_extension(p, ext)))
            .collect();
        out.sort();
        return Ok(out);
    }
    Err(anyhow!(
        "path {} is neither a file nor a directory",
        path.display()
    ))
}

/// Returns true when `p`'s extension equals `ext` (case-insensitive).
pub fn has_extension(p: &Path, ext: &str) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(ext))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_file_returns_itself() {
        let f = std::env::temp_dir().join(format!("opendqi-discover-{}.csv", std::process::id()));
        std::fs::write(&f, "a,b\n1,2\n").unwrap();
        let out = discover_emir_inputs(&f).unwrap();
        assert_eq!(out, vec![f.clone()]);
        std::fs::remove_file(&f).unwrap();
    }

    #[test]
    fn directory_lists_csv_and_xml_sorted() {
        let dir = std::env::temp_dir().join(format!("opendqi-discover-dir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("b.csv"), "x\n").unwrap();
        std::fs::write(dir.join("a.xml"), "<root/>").unwrap();
        std::fs::write(dir.join("c.csv"), "x\n").unwrap();
        std::fs::write(dir.join("ignore.txt"), "x\n").unwrap();
        let out = discover_emir_inputs(&dir).unwrap();
        let names: Vec<_> = out
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["a.xml", "b.csv", "c.csv"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn zip_input_returns_clear_error() {
        let f = std::env::temp_dir().join(format!("opendqi-discover-{}.zip", std::process::id()));
        std::fs::write(&f, b"PK\x03\x04").unwrap();
        let err = discover_emir_inputs(&f).unwrap_err();
        assert!(err.to_string().contains("archives"));
        std::fs::remove_file(&f).unwrap();
    }
}
