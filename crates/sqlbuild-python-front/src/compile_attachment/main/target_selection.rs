//! The selected target name of a discovered project and its effective config, if any.

use crate::contracts::main::resolve_target_config::resolve_target_config;
use crate::contracts::main::resolve_target_name::resolve_target_name;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The selected target name of a discovered project and its effective config, if any.
pub fn target_selection<'py>(
    discovered: &Bound<'py, PyAny>,
    selected: Option<Bound<'py, PyAny>>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = discovered.py();
    let project_config: Bound<'py, PyAny> = native_field(discovered, "project_config")?;
    let local_config: Bound<'py, PyAny> = native_field(discovered, "local_config")?;
    let selected: Bound<'py, PyAny> = selected.unwrap_or_else(|| py.None().into_bound(py));
    let Some(name) = resolve_target_name(py, &project_config, &local_config, &selected)? else {
        return Ok((py.None().into_bound(py), py.None().into_bound(py)));
    };
    let name: Bound<'py, PyAny> = name.into_any();
    let config: Bound<'py, PyAny> =
        resolve_target_config(py, &project_config, &local_config, &name)?;
    Ok((name, config))
}
