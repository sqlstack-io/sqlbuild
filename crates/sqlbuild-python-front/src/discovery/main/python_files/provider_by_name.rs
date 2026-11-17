//! `{provider.name: provider for provider in providers}`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `{provider.name: provider for provider in providers}`.
pub fn provider_by_name<'py>(providers: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let by_name: Bound<'py, PyDict> = PyDict::new(providers.py());
    for provider in providers.try_iter()? {
        let provider: Bound<'py, PyAny> = provider?;
        by_name.set_item(native_field(&provider, "name")?, provider)?;
    }
    Ok(by_name)
}
