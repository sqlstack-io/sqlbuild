//! Resolve scenario config, allowing local overrides to replace project rules.

use crate::contracts::_helpers::spec_resolution::SNAPSHOT_LIMITS;
use crate::contracts::_helpers::spec_resolution::override_or;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Resolve scenario config, allowing local overrides to replace project rules.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config))]
pub fn resolve_effective_scenario_config<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_config.py();
    let project: Bound<'py, PyAny> = native_field(project_config, "scenario")?;
    let local: Bound<'py, PyAny> = native_field(local_config, "scenario")?;
    let overrides: Bound<'py, PyDict> = PyDict::new(py);
    for item in native_field(&project, "local_type_overrides")?
        .call_method0("items")?
        .try_iter()?
    {
        let (dialect, rules): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        overrides.set_item(dialect, py.get_type::<PyDict>().call1((rules,))?)?;
    }
    for item in native_field(&local, "local_type_overrides")?
        .call_method0("items")?
        .try_iter()?
    {
        let (dialect, rules): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        overrides
            .call_method1("setdefault", (dialect, PyDict::new(py)))?
            .call_method1("update", (rules,))?;
    }
    let project_limits: Bound<'py, PyAny> = native_field(&project, "snapshot_limits")?;
    let local_limits: Bound<'py, PyAny> = native_field(&local, "snapshot_limits")?;
    let limits: Vec<(&str, Bound<'py, PyAny>)> = SNAPSHOT_LIMITS
        .iter()
        .map(|field| {
            Ok((
                *field,
                override_or(
                    native_field(&local_limits, field)?,
                    native_field(&project_limits, field)?,
                ),
            ))
        })
        .collect::<PyResult<_>>()?;
    new_record_with(
        py,
        RecordId::ScenarioConfig,
        &[
            (
                "run_namespace",
                override_or(
                    native_field(&local, "run_namespace")?,
                    native_field(&project, "run_namespace")?,
                ),
            ),
            ("local_type_overrides", overrides.into_any()),
            (
                "snapshot_limits",
                new_record_with(py, RecordId::ScenarioSnapshotLimitsConfig, &limits)?,
            ),
        ],
    )
}
