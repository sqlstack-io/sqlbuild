//! The native lifecycle event catalogue: known types, terminality and catalogued values.

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyString;
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
pub(crate) fn lifecycle_event_catalogued(
    schema_version: &Bound<'_, PyAny>,
    event_type: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    Ok(definition(schema_version, event_type)?.is_some())
}

/// Whether a catalogued event closes its correlated lifecycle scope; `KeyError` otherwise.
#[pyfunction]
#[pyo3(signature = (*, schema_version, event_type))]
pub(crate) fn lifecycle_event_terminal(
    schema_version: &Bound<'_, PyAny>,
    event_type: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    definition(schema_version, event_type)?
        .map(|entry| entry.terminal)
        .ok_or_else(|| PyKeyError::new_err(event_type.clone().unbind()))
}
