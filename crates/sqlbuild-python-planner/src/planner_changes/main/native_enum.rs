//! A member of a natively declared planner enumeration, from its value.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;

/// A member of a natively declared planner enumeration, from its value.
pub fn native_enum<'py>(
    py: Python<'py>,
    name: RecordId,
    value: &str,
) -> PyResult<Bound<'py, PyAny>> {
    new_record(py, name, (value,), None)
}
