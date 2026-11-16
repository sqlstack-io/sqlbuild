//! `dataclasses.replace(value, **changes)`.

use crate::compiled_views::_helpers::project::records::DATACLASSES_MODULE;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::boundary::main::imported::imported;

/// `dataclasses.replace(value, **changes)`.
pub fn replace<'py>(
    value: &Bound<'py, PyAny>,
    changes: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    for (name, change) in changes {
        keywords.set_item(*name, change)?;
    }
    imported(py, DATACLASSES_MODULE, "replace")?.call((value,), Some(&keywords))
}
