//! The native subscriber that publishes every lifecycle fact and artifact into Recall.

use crate::recall::main::escaped_text::escaped_text;
use std::collections::HashMap;

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use serde_json::{Map, Value};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_recall::records::main::parse_event::parse_event;

use crate::recall::_helpers::events::event_object;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

/// `event` as the JSON object `lifecycle_event_to_json` encodes; `None` when it cannot be encoded.
pub(crate) fn lifecycle_event_object(
    py: Python<'_>,
    event: &Bound<'_, PyAny>,
) -> PyResult<Option<Map<String, Value>>> {
    if let Some(object) = event_object(event)? {
        return Ok(Some(object));
    }
    let encoded = host_entry(py, ExecutorHost::LifecycleEventToJson)?.call1((event,))?;
    let text = escaped_text(encoded.cast::<PyString>()?)?;
    match parse_event(&text) {
        Ok(object) => Ok(Some(object)),
        Err(_unencodable) => Ok(None),
    }
}

/// The SQL and resource of a statement that has not reached its terminal event yet.
pub(crate) type RunningSql = HashMap<String, (String, Option<String>)>;

/// `outcome` with ordinary exceptions dropped, so recording never changes what the command raises; interrupts still propagate.
pub(crate) fn recording(py: Python<'_>, outcome: PyResult<()>) -> PyResult<()> {
    match outcome {
        Err(error) if error.is_instance_of::<PyException>(py) => Ok(()),
        other => other,
    }
}
