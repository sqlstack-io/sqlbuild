//! Gather existing relations from the deferred target's relation-location schemas.

use crate::compile_driver::_helpers::stages::deferral::deferred_relations;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Gather existing relations from the deferred target's relation-location schemas.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn gather_deferred_relations<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "gather_deferred_relations",
        options,
        (&["adapter", "connection", "deferred_locations"], &[]),
    )?;
    deferred_relations(
        &arguments.value("adapter")?,
        &arguments.value("connection")?,
        &arguments.value("deferred_locations")?,
    )
}
