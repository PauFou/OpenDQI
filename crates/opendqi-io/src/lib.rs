//! OpenDQI I/O: CSV ingestion, YAML mapping, file discovery.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod csv_in;
pub mod discover;
pub mod mapping;
pub mod parquet_in;
pub mod parquet_out;

pub use csv_in::{read_emir_csv, read_sftr_csv};
pub use discover::{discover_emir_inputs, has_extension};
pub use mapping::CsvMapping;
pub use parquet_in::{read_emir_parquet, read_sftr_parquet};
pub use parquet_out::{write_emir_parquet, write_sftr_parquet};
