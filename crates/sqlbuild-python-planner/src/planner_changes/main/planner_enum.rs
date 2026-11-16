//! `TYPES_MODULE.<enum>(value)`.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;

/// `TYPES_MODULE.<enum>(value)`.
pub fn planner_enum<'py>(
    py: Python<'py>,
    name: RecordId,
    value: &str,
) -> PyResult<Bound<'py, PyAny>> {
    new_record(py, name, (value,), None)
}
