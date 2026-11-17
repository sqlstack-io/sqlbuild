//! Plan uncached SQL tests and report their errors without writing artifacts or cache.

use crate::compile_command::_helpers::artifacts::target_writer::PlannedStaticSqlTests;
use crate::compile_command::_helpers::artifacts::target_writer::artifact_diagnostics;
use crate::compile_command::_helpers::artifacts::target_writer::resolved_plan;
use crate::compile_command::_helpers::artifacts::target_writer::zipped;
use crate::compile_command::models::Prepared;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyTuple;

/// Plan uncached SQL tests and report their errors without writing artifacts or cache.
pub fn planning_diagnostics<'py>(
    target_dir: &Bound<'py, PyAny>,
    (adapter, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    prepared: Prepared<'_, 'py>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project.py();
    let planned: Bound<'py, PlannedStaticSqlTests> =
        resolved_plan(target_dir, (adapter, project), prepared)?;
    let mut diagnostics: Vec<Bound<'py, PyAny>> = Vec::new();
    for ((test, _, _), artifact) in zipped(planned.get())? {
        diagnostics.extend(artifact_diagnostics(test.bind(py), artifact)?);
    }
    PyTuple::new(py, diagnostics)
}
