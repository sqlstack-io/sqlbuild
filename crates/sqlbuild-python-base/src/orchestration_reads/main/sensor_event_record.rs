//! The event a sensor's function is called with, as the record its condition's functions receive.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use serde_json::Value;
use sqlbuild_orchestration::sensors::models::SensorEvent;

use crate::boundary::main::imported::imported;
use crate::boundary::main::json_value_to_python::json_value_to_python;
use crate::orchestration_reads::constants::{SENSOR_EVENTS_MODULE, SENSOR_KINDS_MODULE};

/// The event as the record its condition's functions receive, such as a `RunEvent`.
pub fn sensor_event_record<'py>(
    py: Python<'py>,
    event: Option<&SensorEvent>,
) -> PyResult<Bound<'py, PyAny>> {
    let Some(event) = event else {
        return Ok(py.None().into_bound(py));
    };
    let mut fields: Value =
        serde_json::to_value(event).map_err(|error| PyValueError::new_err(error.to_string()))?;
    let (class, kinds): (&str, &[(&str, &str)]) = match event {
        SensorEvent::Run { .. } => ("RunEvent", &[("status", "RunEventKind")]),
        SensorEvent::Node { .. } => ("NodeEvent", &[("event", "NodeEventKind")]),
        SensorEvent::Check { .. } => ("CheckEvent", &[("status", "CheckEventKind")]),
        SensorEvent::SourceNewData { .. } => ("SourceNewDataEvent", &[]),
        SensorEvent::SourceStale { .. } => ("SourceStaleEvent", &[]),
        SensorEvent::Platform { .. } => ("PlatformEvent", &[("event", "PlatformEventKind")]),
    };
    if let Value::Object(map) = &mut fields {
        map.remove("kind");
    }
    let record = json_value_to_python(py, &fields)?.cast_into::<PyDict>()?;
    let kinds_module = py.import(SENSOR_KINDS_MODULE)?;
    for (field, kind) in kinds {
        if let Some(value) = record.get_item(field)? {
            record.set_item(field, kinds_module.getattr(*kind)?.call1((value,))?)?;
        }
    }
    let parse = imported(py, "datetime", "datetime")?.getattr("fromisoformat")?;
    if let Some(occurred_at) = record.get_item("occurred_at")? {
        record.set_item("occurred_at", parse.call1((occurred_at,))?)?;
    }
    for field in ["failed_nodes", "failed_checks"] {
        if let Some(items) = record.get_item(field)? {
            let items: Vec<Bound<'_, PyAny>> = items.try_iter()?.collect::<PyResult<_>>()?;
            record.set_item(field, PyTuple::new(py, items)?)?;
        }
    }
    py.import(SENSOR_EVENTS_MODULE)?
        .getattr(class)?
        .call((), Some(&record))
}
