//! Gather source inspection.

use crate::planner_warehouse::_helpers::sources::source_columns::source_inspection;
<<<<<<< HEAD
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyTuple;

pub fn gather_source_inspection<'py>(
    project: &Bound<'py, PyAny>,
    reads: &Bound<'py, PlannerReads>,
    source_entries: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let (columns, listed) = source_inspection(project, reads, source_entries)?;
=======
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyTuple;

/// Gather source inspection.
pub fn gather_source_inspection<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    source_entries: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let (columns, listed) = source_inspection(project, (adapter, connection), source_entries)?;
>>>>>>> origin/main
    Ok(PyTuple::new(project.py(), [columns.into_any(), listed])?.into_any())
}
