//! The CSV reader settings a seed declaration authors, without the dialect check.

use crate::discovery::_helpers::project::project_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_discovery::project_validation::models::CsvSettings;

/// The CSV reader settings a seed declaration authors, without the dialect check.
#[inline]
pub fn seed_csv_settings(entry: &Bound<'_, PyAny>) -> PyResult<CsvSettings> {
    project_facts::seed_csv_settings(entry)
}
