//! `{provider.name: provider for provider in providers}`.

use crate::discovery::_helpers::python_files::imported_modules;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// `{provider.name: provider for provider in providers}`.
#[inline]
pub fn provider_by_name<'py>(providers: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    imported_modules::provider_by_name(providers)
}
