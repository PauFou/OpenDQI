//! Input discovery: turn a path (file or directory) into a list of input files.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};

/// Extensions accepted by `discover_emir_inputs`.
const SUPPORTED_EXTENSIONS: &[&str] = &["csv", "xml", "parquet"];

/// Discover EMIR inputs at `path`.
///
/// - A single `csv` / `xml` / `parquet` file is returned as-is.
/// - A `.zip` archive is transparently extracted: only its
///   `csv` / `xml` / `parquet` members are returned (directory
///   components in member names are dropped — no zip-slip), sorted.
/// - A single-stream `.gz` (e.g. `foo.csv.gz`) is decompressed to its
///   inner file and that path is returned.
/// - A directory is scanned non-recursively for supported files,
///   sorted by name for deterministic output.
///
/// Archive members and decompressed streams are written to a per-run
/// temp directory under `$TMPDIR`, reclaimed by the OS on reboot —
/// the same lifetime contract as `opendqi desktop`.
pub fn discover_emir_inputs(path: &Path) -> Result<Vec<PathBuf>> {
    let meta = std::fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    if meta.is_file() {
        if has_extension(path, "zip") {
            return extract_zip(path);
        }
        if has_extension(path, "gz") {
            return Ok(vec![extract_gz(path)?]);
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

/// A per-run scratch directory under `$TMPDIR`. No explicit cleanup:
/// the OS reclaims `$TMPDIR` on reboot — identical to the `opendqi
/// desktop` upload-staging contract. pid + nanos keeps concurrent
/// runs isolated without pulling a UUID dependency into this crate.
fn extract_dir() -> Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("opendqi-extract-{}-{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir).with_context(|| format!("create temp dir {}", dir.display()))?;
    Ok(dir)
}

/// Extract the supported members of a `.zip` to a scratch directory.
fn extract_zip(zip_path: &Path) -> Result<Vec<PathBuf>> {
    let file =
        std::fs::File::open(zip_path).with_context(|| format!("open {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("read zip archive {}", zip_path.display()))?;
    let dir = extract_dir()?;
    let mut out: Vec<PathBuf> = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .with_context(|| format!("read entry {i} of {}", zip_path.display()))?;
        if entry.is_dir() {
            continue;
        }
        // Zip-slip guard: only ever trust the bare file name, never
        // the archive-declared path (which may carry `../` traversal
        // or absolute components).
        let name = match Path::new(entry.name()).file_name() {
            Some(n) => n.to_owned(),
            None => continue,
        };
        if !SUPPORTED_EXTENSIONS
            .iter()
            .any(|ext| has_extension(Path::new(&name), ext))
        {
            continue;
        }
        let dest = dir.join(&name);
        let mut sink =
            std::fs::File::create(&dest).with_context(|| format!("create {}", dest.display()))?;
        std::io::copy(&mut entry, &mut sink).with_context(|| {
            format!(
                "extract {} from {}",
                name.to_string_lossy(),
                zip_path.display()
            )
        })?;
        out.push(dest);
    }
    if out.is_empty() {
        return Err(anyhow!(
            "zip {} contains no CSV/XML/Parquet members",
            zip_path.display()
        ));
    }
    out.sort();
    Ok(out)
}

/// Decompress a single-stream `.gz` to a scratch directory. The
/// stripped inner name must carry a supported extension so the
/// caller's by-extension parser dispatch keeps working.
fn extract_gz(gz_path: &Path) -> Result<PathBuf> {
    // `foo.csv.gz` -> `foo.csv`; `data.gz` -> `data` (rejected).
    // `file_stem` drops exactly the final `.gz` regardless of case.
    let inner_name = gz_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            anyhow!(
                "cannot determine inner file name for {}; expected <name>.<ext>.gz",
                gz_path.display()
            )
        })?
        .to_owned();
    if !SUPPORTED_EXTENSIONS
        .iter()
        .any(|ext| has_extension(Path::new(&inner_name), ext))
    {
        return Err(anyhow!(
            "gzip {} decompresses to an unsupported type ({inner_name}); expected a CSV/XML/Parquet file",
            gz_path.display()
        ));
    }
    let file =
        std::fs::File::open(gz_path).with_context(|| format!("open {}", gz_path.display()))?;
    let mut decoder = flate2::read::GzDecoder::new(file);
    let dir = extract_dir()?;
    let dest = dir.join(&inner_name);
    let mut sink =
        std::fs::File::create(&dest).with_context(|| format!("create {}", dest.display()))?;
    std::io::copy(&mut decoder, &mut sink)
        .with_context(|| format!("decompress {}", gz_path.display()))?;
    Ok(dest)
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
    fn gz_input_extracts_and_returns_inner() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let gz = std::env::temp_dir().join(format!("opendqi-d-{}-x.csv.gz", std::process::id()));
        let body = b"uti,notional\nU1,100\n";
        let mut enc = GzEncoder::new(std::fs::File::create(&gz).unwrap(), Compression::default());
        enc.write_all(body).unwrap();
        enc.finish().unwrap();

        let out = discover_emir_inputs(&gz).unwrap();
        assert_eq!(out.len(), 1);
        assert!(has_extension(&out[0], "csv"));
        assert_eq!(std::fs::read(&out[0]).unwrap(), body);

        std::fs::remove_file(&gz).unwrap();
        let _ = std::fs::remove_dir_all(out[0].parent().unwrap());
    }

    #[test]
    fn gz_unsupported_inner_type_errors() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let gz = std::env::temp_dir().join(format!("opendqi-d-{}-note.txt.gz", std::process::id()));
        let mut enc = GzEncoder::new(std::fs::File::create(&gz).unwrap(), Compression::default());
        enc.write_all(b"hello").unwrap();
        enc.finish().unwrap();

        let err = discover_emir_inputs(&gz).unwrap_err();
        assert!(err.to_string().contains("unsupported type"));
        std::fs::remove_file(&gz).unwrap();
    }

    #[test]
    fn zip_input_extracts_supported_members_sorted() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let zp = std::env::temp_dir().join(format!("opendqi-d-{}.zip", std::process::id()));
        let opts = SimpleFileOptions::default();
        let mut zw = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
        zw.start_file("b.csv", opts).unwrap();
        zw.write_all(b"x\n").unwrap();
        zw.start_file("nested/a.xml", opts).unwrap();
        zw.write_all(b"<root/>").unwrap();
        zw.start_file("note.txt", opts).unwrap();
        zw.write_all(b"ignored").unwrap();
        zw.finish().unwrap();

        let out = discover_emir_inputs(&zp).unwrap();
        let names: Vec<_> = out
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["a.xml", "b.csv"]);
        assert_eq!(std::fs::read_to_string(&out[0]).unwrap(), "<root/>");
        assert_eq!(std::fs::read_to_string(&out[1]).unwrap(), "x\n");

        std::fs::remove_file(&zp).unwrap();
        let _ = std::fs::remove_dir_all(out[0].parent().unwrap());
    }

    #[test]
    fn zip_with_no_supported_members_errors() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let zp = std::env::temp_dir().join(format!("opendqi-d-{}-empty.zip", std::process::id()));
        let mut zw = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
        zw.start_file("note.txt", SimpleFileOptions::default())
            .unwrap();
        zw.write_all(b"nothing here").unwrap();
        zw.finish().unwrap();

        let err = discover_emir_inputs(&zp).unwrap_err();
        assert!(err.to_string().contains("no CSV/XML/Parquet"));
        std::fs::remove_file(&zp).unwrap();
    }

    #[test]
    fn invalid_zip_errors() {
        let f = std::env::temp_dir().join(format!("opendqi-d-{}-bad.zip", std::process::id()));
        std::fs::write(&f, b"PK\x03\x04").unwrap();
        // A truncated archive must fail loudly, not silently yield no
        // inputs.
        assert!(discover_emir_inputs(&f).is_err());
        std::fs::remove_file(&f).unwrap();
    }
}
