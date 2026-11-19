//! The command's job and job group names, loaded on first use; empty outside a command.

use crate::job_settings::_helpers::job_catalog::job_catalog;
use pyo3::prelude::{PyResult, Python};

/// The command's job and job group names, loaded on first use; empty outside a command.
pub fn job_names(py: Python<'_>) -> PyResult<Vec<String>> {
    Ok(job_catalog(py)?
        .map(|(catalog, _)| catalog.into_keys().collect())
        .unwrap_or_default())
}
