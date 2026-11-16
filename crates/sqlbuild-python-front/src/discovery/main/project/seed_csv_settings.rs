//! The CSV reader settings a seed declaration authors, without the dialect check.

use crate::discovery::_helpers::project::project_facts::optional_text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_discovery::project_validation::models::CsvSettings;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The CSV reader settings a seed declaration authors, without the dialect check.
pub fn seed_csv_settings(entry: &Bound<'_, PyAny>) -> PyResult<CsvSettings> {
    let settings: Bound<'_, PyAny> = native_field(entry, "csv_settings")?;
    Ok(CsvSettings {
        delimiter: optional_text(&settings, "delimiter")?,
        quotechar: optional_text(&settings, "quotechar")?,
        escapechar: optional_text(&settings, "escapechar")?,
        doublequote: native_field(&settings, "doublequote")?.extract()?,
        skipinitialspace: native_field(&settings, "skipinitialspace")?.extract()?,
        dialect_error: None,
    })
}
