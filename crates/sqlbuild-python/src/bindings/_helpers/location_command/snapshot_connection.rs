//! The target's non-secret connection settings, published with each snapshot for the Connections page.

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use serde_json::{Map, Value};
use sqlbuild_orchestration::snapshots::constants::CONNECTION_DOCUMENT;
use sqlbuild_orchestration::snapshots::main::reads::connection_details::connection_details;
use sqlbuild_orchestration::snapshots::models::SnapshotInput;

use crate::bindings::_helpers::command_runtime::connection::project_connection_config;
use crate::bindings::_helpers::command_runtime::warehouse_scope::command_warehouse_scope;
use crate::bindings::_helpers::query_command::project::ProjectAdapter;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::records::main::fields::field::field;

/// The resolved connection's text settings; only `connection_details` decides which are published.
fn text_settings(
    project: &ProjectAdapter<'_>,
    selected_target: &Bound<'_, PyAny>,
) -> PyResult<Map<String, Value>> {
    let py: Python<'_> = selected_target.py();
    let none: Bound<'_, PyAny> = py.None().into_bound(py);
    let unscoped: Bound<'_, PyAny> =
        Bound::new(py, command_warehouse_scope(&none, &none, None)?)?.into_any();
    let config = within(&unscoped, |_| {
        project_connection_config(
            &project.discovered,
            (&project.project_dir, selected_target),
            (&none, false),
            false,
        )
    })?;
    let mut settings: Map<String, Value> = Map::new();
    for (key, value) in config.cast_into::<PyDict>()?.iter() {
        let shown = if value.is_none() {
            Value::Null
        } else if value.is_instance_of::<PyString>() {
            Value::String(value.extract()?)
        } else {
            Value::Bool(true)
        };
        settings.insert(key.extract()?, shown);
    }
    Ok(settings)
}

/// Add the `connection` document; a connection that cannot be resolved here, such as one naming an unset secret, publishes none.
pub(super) fn add_connection(
    mut input: SnapshotInput,
    (project, selected_target): (&ProjectAdapter<'_>, &Bound<'_, PyAny>),
) -> PyResult<SnapshotInput> {
    let py: Python<'_> = selected_target.py();
    let settings = match text_settings(project, selected_target) {
        Ok(settings) => settings,
        Err(error) if error.is_instance_of::<PyException>(py) => return Ok(input),
        Err(error) => return Err(error),
    };
    let adapter: String = field(&project.adapter, "adapter_name")?.str()?.extract()?;
    let details = serde_json::to_value(connection_details(&adapter, &settings))
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
    input.documents.insert(CONNECTION_DOCUMENT.into(), details);
    Ok(input)
}
