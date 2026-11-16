//! The native lifecycle event catalogue: known types, terminality and catalogued values.

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::types::{PyFrozenSet, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::lifecycle_events::constants::{
    CURRENT_DIAGNOSTIC_LOG_SCHEMA_VERSION, CURRENT_LIFECYCLE_EVENT_SCHEMA_VERSION, HOOK_PHASES,
    HOOK_TYPES, LIFECYCLE_EVENT_SCHEMA_VERSIONS, OPERATION_ADAPTERS, OPERATION_KINDS,
    OPERATION_METADATA_FIELDS, OPERATION_NAMES, OPERATION_PHASES, OPERATION_SCOPES,
    OPERATION_STRATEGIES, OPERATION_TARGET_KINDS, RESOURCE_SKIP_CODES, RESOURCE_SKIP_MODES,
};
use sqlbuild_core::lifecycle_events::main::catalog::catalog;
use sqlbuild_core::lifecycle_events::models::EventDefinition;

/// The definition of `event_type` in the `schema_version` catalogue, if both are catalogued.
pub(crate) fn definition(
    schema_version: &Bound<'_, PyAny>,
    event_type: &Bound<'_, PyAny>,
) -> PyResult<Option<EventDefinition>> {
    let (Ok(version), Ok(name)) = (
        schema_version.extract::<i64>(),
        event_type.downcast::<PyString>(),
    ) else {
        return Ok(None);
    };
    let name: String = name.extract()?;
    Ok(catalog(version)
        .and_then(|entries| entries.into_iter().find(|entry| entry.event_type == name)))
}

/// Whether `event_type` is catalogued for `schema_version`.
#[pyfunction]
#[pyo3(signature = (*, schema_version, event_type))]
fn lifecycle_event_catalogued(
    schema_version: &Bound<'_, PyAny>,
    event_type: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    Ok(definition(schema_version, event_type)?.is_some())
}

/// Whether a catalogued event closes its correlated lifecycle scope; `KeyError` otherwise.
#[pyfunction]
#[pyo3(signature = (*, schema_version, event_type))]
fn lifecycle_event_terminal(
    schema_version: &Bound<'_, PyAny>,
    event_type: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    definition(schema_version, event_type)?
        .map(|entry| entry.terminal)
        .ok_or_else(|| PyKeyError::new_err(event_type.clone().unbind()))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    module.add(
        "CURRENT_LIFECYCLE_EVENT_SCHEMA_VERSION",
        CURRENT_LIFECYCLE_EVENT_SCHEMA_VERSION,
    )?;
    module.add(
        "CURRENT_DIAGNOSTIC_LOG_SCHEMA_VERSION",
        CURRENT_DIAGNOSTIC_LOG_SCHEMA_VERSION,
    )?;
    module.add(
        "LIFECYCLE_EVENT_SCHEMA_VERSIONS",
        PyFrozenSet::new(py, LIFECYCLE_EVENT_SCHEMA_VERSIONS)?,
    )?;
    let sets: [(&str, &[&str]); 12] = [
        ("HOOK_PHASES", &HOOK_PHASES),
        ("HOOK_TYPES", &HOOK_TYPES),
        ("OPERATION_ADAPTERS", &OPERATION_ADAPTERS),
        ("OPERATION_KINDS", &OPERATION_KINDS),
        ("OPERATION_METADATA_FIELDS", &OPERATION_METADATA_FIELDS),
        ("OPERATION_NAMES", &OPERATION_NAMES),
        ("OPERATION_PHASES", &OPERATION_PHASES),
        ("OPERATION_SCOPES", &OPERATION_SCOPES),
        ("OPERATION_STRATEGIES", &OPERATION_STRATEGIES),
        ("OPERATION_TARGET_KINDS", &OPERATION_TARGET_KINDS),
        ("RESOURCE_SKIP_CODES", &RESOURCE_SKIP_CODES),
        ("RESOURCE_SKIP_MODES", &RESOURCE_SKIP_MODES),
    ];
    for (name, values) in sets {
        module.add(name, PyFrozenSet::new(py, values)?)?;
    }
    module.add_function(wrap_pyfunction!(lifecycle_event_catalogued, module)?)?;
    module.add_function(wrap_pyfunction!(lifecycle_event_terminal, module)?)
}
