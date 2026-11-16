//! `dataclasses.replace(value, **changes)`.

use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;

/// `dataclasses.replace(value, **changes)`.
pub fn replaced(
    _py: Python<'_>,
    value: &Bound<'_, PyAny>,
    changes: &Bound<'_, PyDict>,
) -> PyResult<Py<PyAny>> {
    Ok(replaced_keywords(value, changes)?.unbind())
}
