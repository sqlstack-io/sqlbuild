//! Model-level cursor safety policy overrides over project configuration.

use crate::bindings::_helpers::model_config::typed_config::{config_value, model_values};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyString;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const POLICY_DISABLED: &str = "disabled";

/// The policy class, its distance field, and the model keys overriding distance and action.
struct PolicyFields {
    class: RecordId,
    distance: &'static str,
    distance_key: &'static str,
    action_key: &'static str,
}

const FUTURE_POLICY: PolicyFields = PolicyFields {
    class: RecordId::FutureCursorsConfig,
    distance: "max_distance",
    distance_key: "cursor_future_max_distance",
    action_key: "cursor_future_action",
};

const START_POLICY: PolicyFields = PolicyFields {
    class: RecordId::StartCursorsConfig,
    distance: "max_ahead",
    distance_key: "cursor_start_max_ahead",
    action_key: "cursor_start_max_action",
};

fn resolve_policy<'py>(
    model: &Bound<'py, PyAny>,
    project_config: &Bound<'py, PyAny>,
    fields: &PolicyFields,
) -> PyResult<Bound<'py, PyAny>> {
    let py = model.py();
    let class = record_class(py, fields.class)?;
    let configured = if project_config.is_truthy()? {
        project_config.clone()
    } else {
        class.call0()?
    };
    let values = model_values(model)?;
    let raw_distance = config_value(py, &values, fields.distance_key)?;
    let raw_action = config_value(py, &values, fields.action_key)?;
    let mut distance = configured.getattr(fields.distance)?;
    if raw_distance.is_instance_of::<PyString>() {
        distance = if raw_distance.eq(POLICY_DISABLED)? {
            py.None().into_bound(py)
        } else {
            raw_distance
        };
    }
    let mut action = native_field(&configured, "action")?;
    if raw_action.is_instance_of::<PyString>() {
        action = enum_from_value(py, RecordId::FutureCursorAction, raw_action)?;
    }
    if distance.is_none() {
        return Ok(py.None().into_bound(py));
    }
    call_with(&class, &[(fields.distance, distance), ("action", action)])
}

/// The future-end policy for one model, model configuration taking precedence.
pub(crate) fn future_cursor_config<'py>(
    model: &Bound<'py, PyAny>,
    project_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    resolve_policy(model, project_config, &FUTURE_POLICY)
}

/// The automatic-start policy for one model, model configuration taking precedence.
pub(crate) fn start_cursor_config<'py>(
    model: &Bound<'py, PyAny>,
    project_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    resolve_policy(model, project_config, &START_POLICY)
}

/// Resolve automatic-start policy with model config taking precedence.
pub(crate) fn resolve_start_cursor_config<'py>(
    model: &Bound<'py, PyAny>,
    project_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    start_cursor_config(model, project_config)
}
