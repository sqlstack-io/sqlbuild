//! `dataclasses.replace(value, **keywords)`: a native record is rebuilt from its own values.

use crate::records::_helpers::native_classes::fields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// `dataclasses.replace(value, **keywords)`: a native record is rebuilt from its own values.
#[inline]
pub fn replaced_keywords<'py>(
    value: &Bound<'py, PyAny>,
    keywords: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    fields::replaced_keywords(value, keywords)
}
