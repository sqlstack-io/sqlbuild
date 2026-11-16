//! The values of a table-backed `MaterializationType`, as text.

use crate::input_loops::_helpers::resources::loop_classes::TABLE_BACKED_MATERIALIZATIONS;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::types::RecordId;

/// The values of a table-backed `MaterializationType`, as text.
pub fn table_backed<'py>(py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    TABLE_BACKED_MATERIALIZATIONS
        .iter()
        .map(|name| enum_member(py, RecordId::MaterializationType, name))
        .collect()
}
