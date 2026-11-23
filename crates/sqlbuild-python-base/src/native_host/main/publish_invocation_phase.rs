//! `publish_invocation_phase`: one finished phase as a lifecycle event of the current invocation.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::boundary::main::call_with::call_with;
use crate::invocation_phases::constants::INVOCATION_PHASE_EVENT;
use crate::invocation_phases::models::InvocationPhase;
use crate::native_host::main::host_entry::host_entry;

/// Publish `phase` to the current event dispatcher; without one, nothing is published.
pub fn publish_invocation_phase(py: Python<'_>, phase: &InvocationPhase<'_>) -> PyResult<()> {
    let dispatcher: Bound<'_, PyAny> =
        host_entry(py, ExecutorHost::CurrentEventDispatcher)?.call0()?;
    if dispatcher.is_none() {
        return Ok(());
    }
    let payload: Bound<'_, PyDict> = PyDict::new(py);
    payload.set_item("phase", phase.phase)?;
    if let Some(duration_ms) = phase.duration_ms {
        payload.set_item("duration_ms", duration_ms.max(0.0))?;
    }
    if let Some(count) = phase.count {
        payload.set_item("count", count)?;
    }
    for (key, value) in [("name", &phase.name), ("detail", &phase.detail)] {
        if let Some(value) = value {
            payload.set_item(key, value)?;
        }
    }
    let event: Bound<'_, PyAny> = call_with(
        &host_entry(py, ExecutorHost::CreateLifecycleEvent)?,
        &[
            (
                "event_type",
                PyString::new(py, INVOCATION_PHASE_EVENT).into_any(),
            ),
            ("payload", payload.into_any()),
        ],
    )?;
    dispatcher.call_method1("publish_lifecycle", (event,))?;
    Ok(())
}
