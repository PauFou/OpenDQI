//! IO ingestion robustness (see `docs/reliability.md`).
//!
//! `discover_emir_inputs` (zip / gzip / dir routing — zip-slip
//! correctness is unit-tested in `discover.rs`), the Parquet readers,
//! the CSV readers and `CsvMapping::from_path` must never panic / hang
//! on garbage or deterministically-mutated input. Dependency-free,
//! fixed-seed.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use opendqi_io::{
    discover_emir_inputs, read_emir_csv, read_emir_parquet, read_sftr_csv, read_sftr_parquet,
    CsvMapping,
};

fn quiet_panics() {
    std::panic::set_hook(Box::new(|_| {}));
}

fn write_tmp(bytes: &[u8], ext: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "opendqi-robust-io-{}-{}.{ext}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&p, bytes).unwrap();
    p
}

fn ws() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

fn no_panic(label: &str, path: &Path, f: &dyn Fn(&Path)) {
    let r = catch_unwind(AssertUnwindSafe(|| f(path)));
    assert!(r.is_ok(), "PANIC: `{label}` panicked on hostile input");
}

fn bounded(label: &str, path: PathBuf, f: fn(&Path)) {
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let r = catch_unwind(AssertUnwindSafe(|| f(&path)));
        let _ = tx.send(r.is_ok());
    });
    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(true) => {}
        Ok(false) => panic!("PANIC: `{label}` panicked on hostile input"),
        Err(_) => panic!("HANG: `{label}` exceeded the 15s bound on hostile input"),
    }
    let _ = worker.join();
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

#[test]
fn discover_survives_garbage_archives() {
    quiet_panics();
    let big = 4 * 1024 * 1024;
    let cases: Vec<(&str, &str, Vec<u8>)> = vec![
        ("empty.zip", "zip", vec![]),
        (
            "garbage.zip",
            "zip",
            b"PK\x03\x04 not really a zip".to_vec(),
        ),
        ("truncated.zip", "zip", b"PK\x03\x04".to_vec()),
        (
            "randombytes.zip",
            "zip",
            (0u8..=255).cycle().take(8192).collect(),
        ),
        ("empty.gz", "gz", vec![]),
        ("badheader.gz", "gz", b"\x1f\x8b not a gzip stream".to_vec()),
        (
            "randombytes.gz",
            "gz",
            (0u8..=255).cycle().take(8192).collect(),
        ),
        (
            "bigrandom.zip",
            "zip",
            (0u8..=255).cycle().take(big).collect(),
        ),
        ("nonexistent", "xml", b"<ignored/>".to_vec()),
    ];
    for (name, ext, bytes) in cases {
        let p = write_tmp(&bytes, ext);
        let label = format!("discover_emir_inputs / {name}");
        if bytes.len() > 1024 * 1024 {
            bounded(&label, p.clone(), |q| {
                let _ = discover_emir_inputs(q);
            });
        } else {
            no_panic(&label, &p, &|q| {
                let _ = discover_emir_inputs(q);
            });
        }
        let _ = std::fs::remove_file(&p);
    }
    // A path that does not exist must Err cleanly, not panic.
    no_panic(
        "discover_emir_inputs / missing-path",
        Path::new("/opendqi/definitely/not/here.zip"),
        &|q| {
            let _ = discover_emir_inputs(q);
        },
    );
}

#[test]
fn parquet_readers_survive_garbage() {
    quiet_panics();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", vec![]),
        ("not-parquet", b"this is not a parquet file".to_vec()),
        ("fake-magic", b"PAR1 garbage in the middle PAR1".to_vec()),
        ("randombytes", (0u8..=255).cycle().take(64 * 1024).collect()),
    ];
    for (name, bytes) in cases {
        let p = write_tmp(&bytes, "parquet");
        no_panic(&format!("read_emir_parquet / {name}"), &p, &|q| {
            let _ = read_emir_parquet(q);
        });
        no_panic(&format!("read_sftr_parquet / {name}"), &p, &|q| {
            let _ = read_sftr_parquet(q);
        });
        let _ = std::fs::remove_file(&p);
    }
}

#[test]
fn csv_mapping_and_readers_survive_garbage() {
    quiet_panics();
    let big = 4 * 1024 * 1024;

    // CsvMapping::from_path on hostile YAML.
    let yaml_cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", vec![]),
        ("not-yaml", b"\x00\xff not yaml : : :".to_vec()),
        ("scalar", b"just a scalar string".to_vec()),
        ("deep-nested", {
            let mut s = String::new();
            for i in 0..2000 {
                s.push_str(&" ".repeat(i % 8));
                s.push_str("k:\n");
            }
            s.into_bytes()
        }),
        ("huge", vec![b'a'; big]),
    ];
    for (name, bytes) in yaml_cases {
        let p = write_tmp(&bytes, "yml");
        let label = format!("CsvMapping::from_path / {name}");
        if bytes.len() > 1024 * 1024 {
            bounded(&label, p.clone(), |q| {
                let _ = CsvMapping::from_path(q);
            });
        } else {
            no_panic(&label, &p, &|q| {
                let _ = CsvMapping::from_path(q);
            });
        }
        let _ = std::fs::remove_file(&p);
    }

    // CSV readers with a real (valid) mapping, hostile CSV bytes.
    let mapping = CsvMapping::from_path(&ws().join("examples/emir/sample_mapping.yml"))
        .expect("load example mapping");
    let csv_cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", vec![]),
        ("header-only", b"a,b,c\n".to_vec()),
        ("nul-bytes", b"uti,notional\n\x00\x00,\x00\n".to_vec()),
        (
            "unbalanced-quotes",
            b"uti,notional\n\"unterminated,123\n".to_vec(),
        ),
        ("ragged-rows", b"uti\nA,B,C,D,E\n,,,\nX\n".to_vec()),
        ("invalid-utf8", vec![0xff, 0xfe, b'\n', 0x80, b',', 0xc0]),
        ("huge-row", {
            let mut v = b"uti,notional\n".to_vec();
            v.extend(std::iter::repeat_n(b'x', big));
            v.push(b'\n');
            v
        }),
        ("million-columns", {
            let mut v = b"h\n".to_vec();
            v.extend(std::iter::repeat_n(b',', 1_000_000));
            v.push(b'\n');
            v
        }),
    ];
    for (name, bytes) in csv_cases {
        let p = write_tmp(&bytes, "csv");
        let re = catch_unwind(AssertUnwindSafe(|| {
            let _ = read_emir_csv(&p, &mapping);
        }));
        assert!(
            re.is_ok(),
            "PANIC: `read_emir_csv / {name}` panicked on hostile CSV"
        );
        let rs = catch_unwind(AssertUnwindSafe(|| {
            let _ = read_sftr_csv(&p, &mapping);
        }));
        assert!(
            rs.is_ok(),
            "PANIC: `read_sftr_csv / {name}` panicked on hostile CSV"
        );
        let _ = std::fs::remove_file(&p);
    }
}

#[test]
fn deterministic_mutation_of_valid_csv_and_mapping_never_panics() {
    quiet_panics();
    let mapping_path = ws().join("examples/emir/sample_mapping.yml");
    let csv_path = ws().join("examples/emir/sample.csv");
    let mapping = CsvMapping::from_path(&mapping_path).expect("base mapping");
    let mut rng = Lcg(0x5EED_1234_ABCD_0001);

    for (base_path, is_yaml) in [(mapping_path, true), (csv_path, false)] {
        let base = std::fs::read(&base_path).unwrap();
        for _ in 0..150 {
            let mut m = base.clone();
            if m.is_empty() {
                continue;
            }
            for _ in 0..(1 + rng.next() % 4) {
                let i = (rng.next() as usize) % m.len();
                match rng.next() % 3 {
                    0 => m[i] ^= 1 << (rng.next() % 8),
                    1 => m.truncate(i),
                    _ => m.insert(i % (m.len() + 1), (rng.next() & 0xff) as u8),
                }
            }
            let p = write_tmp(&m, if is_yaml { "yml" } else { "csv" });
            let r = catch_unwind(AssertUnwindSafe(|| {
                if is_yaml {
                    let _ = CsvMapping::from_path(&p);
                } else {
                    let _ = read_emir_csv(&p, &mapping);
                    let _ = read_sftr_csv(&p, &mapping);
                }
            }));
            let _ = std::fs::remove_file(&p);
            assert!(
                r.is_ok(),
                "PANIC: mutated {} variant panicked",
                if is_yaml { "mapping" } else { "csv" }
            );
        }
    }
}
