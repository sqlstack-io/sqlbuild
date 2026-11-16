//! Enum and constant declarations as native rows, in the given order.

use crate::compiled_views::_helpers::records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyList;

/// Enum and constant declarations as native rows, in the given order.
#[inline]
pub fn declaration_rows<'py>(
    enums: &Bound<'py, PyAny>,
    constants: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyList>, Bound<'py, PyList>)> {
    records::declaration_rows(enums, constants)
}
