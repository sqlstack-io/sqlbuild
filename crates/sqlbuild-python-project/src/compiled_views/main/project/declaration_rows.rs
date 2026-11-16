//! Enum and constant declarations as native rows, in the given order.

use crate::compiled_views::_helpers::project::records::constant_row;
use crate::compiled_views::_helpers::project::records::enum_row;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyList, PyTuple};

/// Enum and constant declarations as native rows, in the given order.
#[pyfunction]
#[pyo3(signature = (*, enums, constants))]
pub fn declaration_rows<'py>(
    enums: &Bound<'py, PyAny>,
    constants: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyList>, Bound<'py, PyList>)> {
    let py: Python<'py> = enums.py();
    let enum_rows: Vec<Bound<'py, PyTuple>> = enums
        .try_iter()?
        .map(|declaration| enum_row(&declaration?))
        .collect::<PyResult<_>>()?;
    let constant_rows: Vec<Bound<'py, PyTuple>> = constants
        .try_iter()?
        .map(|declaration| constant_row(&declaration?))
        .collect::<PyResult<_>>()?;
    Ok((PyList::new(py, enum_rows)?, PyList::new(py, constant_rows)?))
}
