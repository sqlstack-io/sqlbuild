//! One `ColumnInfo` the planner hands the adapter.

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::boundary::main::call_with::call_with;

/// `ColumnInfo(name=..., type=...)`.
pub(crate) fn column_info<'py>(
    class: &Bound<'py, PyAny>,
    (name, column_type): (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    call_with(class, &[("name", name), ("type", column_type)])
}
