//! `dataclasses.replace(value, **fields)`.

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;

/// `dataclasses.replace(value, **fields)`.
pub fn replace<'py>(
    value: &Bound<'py, PyAny>,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    replaced_keywords(value, &keywords(value.py(), fields)?)
}
