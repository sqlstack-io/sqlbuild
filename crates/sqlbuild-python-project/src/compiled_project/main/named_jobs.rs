//! The jobs a command's selectors name, in the order named.

use crate::compiled_project::_helpers::job_catalog::job_catalog;
use crate::compiled_project::_helpers::job_catalog::ordered_jobs;
use pyo3::prelude::{PyResult, Python};

/// The jobs `selectors` name, as `job:<name>` or a bare job name, each once in the order named; none outside a command.
pub fn named_jobs(py: Python<'_>, selectors: &[String]) -> PyResult<Vec<String>> {
    Ok(job_catalog(py)?
        .map(|(catalog, _)| ordered_jobs(selectors, &catalog))
        .unwrap_or_default())
}
