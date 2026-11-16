//! A planner model dataclass.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

/// A planner model dataclass.
pub fn planner_model<'py>(
    py: Python<'py>,
    name: RecordId,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(py, name, fields)
}
