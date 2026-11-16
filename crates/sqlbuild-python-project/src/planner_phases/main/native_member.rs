//! A native string enum member from its value.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;

/// A native string enum member from its value.
pub fn native_member<'py>(
    py: Python<'py>,
    class: RecordId,
    value: &str,
) -> PyResult<Bound<'py, PyAny>> {
    new_record(py, class, (value,), None)
}
