//! `build_effective_settings`: setting values by field name, local overrides applied.

use crate::contracts::_helpers::effective_config::Key;
use crate::contracts::_helpers::effective_config::Pairs;
use crate::contracts::_helpers::effective_config::dictionary;
use crate::contracts::_helpers::effective_config::key;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, PyStringMethods, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyString};
use sqlbuild_config::effective::main::effective_settings::effective_settings;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `build_effective_settings`: setting values by field name, local overrides applied.
#[pyfunction]
pub fn effective_config_settings<'py>(
    py: Python<'py>,
    settings: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    overrides: &Bound<'py, PyAny>,
    names: Vec<Bound<'py, PyString>>,
) -> PyResult<Bound<'py, PyDict>> {
    let read = |settings: &Bound<'py, PyAny>| -> PyResult<Pairs<'py>> {
        names
            .iter()
            .map(|name| {
                Ok((
                    key(name.as_any())?,
                    native_field(settings, &name.to_cow()?)?,
                ))
            })
            .collect()
    };
    let overridden: Vec<Key<'py>> = overrides
        .try_iter()?
        .map(|name| key(&name?))
        .collect::<PyResult<_>>()?;
    let (project_settings, local_settings) = settings;
    let settings: Pairs<'py> = effective_settings(
        read(&project_settings)?,
        &read(&local_settings)?,
        &overridden,
    );
    dictionary(py, settings)
}
