//! Merge project settings with local developer overrides.

use crate::contracts::main::effective_config_settings::effective_config_settings;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyString};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Merge project settings with local developer overrides.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config))]
pub fn build_effective_settings<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_config.py();
    let settings_class: Bound<'py, PyAny> = record_class(py, RecordId::SettingsConfig)?;
    let names: Vec<Bound<'py, PyAny>> = imported(py, "dataclasses", "fields")?
        .call1((&settings_class,))?
        .try_iter()?
        .map(|field| native_field(&field?, "name"))
        .collect::<PyResult<_>>()?;
    let values: Bound<'py, PyDict> = effective_config_settings(
        py,
        (
            native_field(project_config, "settings")?,
            native_field(local_config, "settings")?,
        ),
        &native_field(local_config, "setting_overrides")?,
        names
            .into_iter()
            .map(|name| Ok(name.cast_into::<PyString>()?))
            .collect::<PyResult<_>>()?,
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
    .cast_into()?;
    new_record(py, RecordId::SettingsConfig, (), Some(&values))
}
