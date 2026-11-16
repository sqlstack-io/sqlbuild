//! Names of the seeds whose CSV holds at most one data row, from `seed_file` and `schema_entry`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_discovery::project_validation::main::at_most_one_csv_data_row::at_most_one_csv_data_row;
use sqlbuild_discovery::project_validation::models::CsvSettings;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::discovery::main::project::seed_csv_settings::seed_csv_settings;
use std::path::PathBuf;

/// Names of the seeds whose CSV holds at most one data row, from `seed_file` and `schema_entry`.
pub fn one_row_seed_names(seeds: &[Bound<'_, PyAny>]) -> PyResult<Vec<String>> {
    let mut candidates: Vec<(String, PathBuf, CsvSettings)> = Vec::with_capacity(seeds.len());
    for seed in seeds {
        let entry: Bound<'_, PyAny> = native_field(seed, "schema_entry")?;
        candidates.push((
            native_field(&entry, "name")?.extract()?,
            native_field(&native_field(seed, "seed_file")?, "file_path")?.extract()?,
            seed_csv_settings(&entry)?,
        ));
    }
    let py: Python<'_> = match seeds.first() {
        Some(seed) => seed.py(),
        None => return Ok(Vec::new()),
    };
    Ok(py.detach(|| {
        candidates
            .into_iter()
            .filter(|(_, path, settings)| at_most_one_csv_data_row(path, settings))
            .map(|(name, _, _)| name)
            .collect()
    }))
}
