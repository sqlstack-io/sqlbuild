//! Gather source columns.

use crate::planner_warehouse::_helpers::sources::source_columns::source_inspection;
<<<<<<< HEAD
use crate::planner_warehouse::models::PlannerReads;
=======
>>>>>>> origin/main
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyDict;

#[pyfunction]
<<<<<<< HEAD
#[pyo3(signature = (*, project, reads, source_entries=None))]
pub fn gather_source_columns<'py>(
    project: &Bound<'py, PyAny>,
    reads: &Bound<'py, PlannerReads>,
    source_entries: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    Ok(source_inspection(project, reads, source_entries)?.0)
=======
#[pyo3(signature = (*, project, adapter, connection, source_entries=None))]
pub fn gather_source_columns<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    source_entries: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    Ok(source_inspection(project, (adapter, connection), source_entries)?.0)
>>>>>>> origin/main
}
