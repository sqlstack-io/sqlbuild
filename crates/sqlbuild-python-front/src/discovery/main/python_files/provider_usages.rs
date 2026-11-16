//! The provider parameters of one loaded user function.

use crate::discovery::_helpers::python_files::imported_modules;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// The provider parameters of one loaded user function.
#[inline]
pub fn provider_usages<'py>(
    function: &Bound<'py, PyAny>,
    provider_by_name: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    imported_modules::provider_usages(function, provider_by_name)
}
