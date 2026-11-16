//! The provider parameters of one loaded user function.

use crate::discovery::_helpers::python_files::imported_modules::shim;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;

/// The provider parameters of one loaded user function.
pub fn provider_usages<'py>(
    function: &Bound<'py, PyAny>,
    provider_by_name: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &shim(function.py(), UserCodeHost::ProviderUsages)?,
        &[
            ("function", function.clone()),
            ("provider_by_name", provider_by_name.clone().into_any()),
        ],
    )
}
