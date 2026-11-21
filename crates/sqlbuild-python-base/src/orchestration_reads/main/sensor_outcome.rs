//! A tick's outcome from what a sensor's function returned, or the reason it may not return that.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_orchestration::sensors::constants::MAX_SENSOR_RUN_REQUESTS;
use sqlbuild_orchestration::sensors::models::{RequestedRun, SensorEvent, SensorFunctionOutcome};

use crate::orchestration_reads::constants::SENSOR_AUTHORING_MODULE;

/// The tick's outcome from the function's return value: `None`, a `SkipReason`, a `RunRequest` or several.
pub fn sensor_outcome(
    py: Python<'_>,
    result: &Bound<'_, PyAny>,
    (event, cursor): (Option<&SensorEvent>, Option<String>),
) -> PyResult<SensorFunctionOutcome> {
    if result.is_none() {
        return Ok(SensorFunctionOutcome::Handled { cursor });
    }
    let authoring = py.import(SENSOR_AUTHORING_MODULE)?;
    if result.is_instance(&authoring.getattr("SkipReason")?)? {
        let reason: String = result.getattr("reason")?.extract()?;
        if reason.trim().is_empty() {
            return Err(PyValueError::new_err("SkipReason needs a nonempty reason"));
        }
        return Ok(SensorFunctionOutcome::Skipped { cursor, reason });
    }
    let request_class = authoring.getattr("RunRequest")?;
    let requests: Vec<Bound<'_, PyAny>> = if result.is_instance(&request_class)? {
        vec![result.clone()]
    } else if result.is_instance_of::<PyString>() {
        return Err(returned_error());
    } else {
        match result.try_iter() {
            Ok(items) => items
                .take(MAX_SENSOR_RUN_REQUESTS + 1)
                .collect::<PyResult<_>>()?,
            Err(_) => return Err(returned_error()),
        }
    };
    if requests.len() > MAX_SENSOR_RUN_REQUESTS {
        return Err(PyValueError::new_err(format!(
            "a sensor may start at most {MAX_SENSOR_RUN_REQUESTS} runs per call"
        )));
    }
    let mut runs: Vec<RequestedRun> = Vec::new();
    for request in requests {
        if !request.is_instance(&request_class)? {
            return Err(returned_error());
        }
        let job: String = request.getattr("job")?.extract()?;
        if job.trim().is_empty() {
            return Err(PyValueError::new_err("RunRequest needs a job"));
        }
        let key: Option<String> = request.getattr("run_key")?.extract()?;
        let run_key = match (key, event) {
            (Some(key), _) if !key.trim().is_empty() => key,
            (_, Some(event)) => format!("{}:{job}", event.id()),
            (_, None) => {
                return Err(PyValueError::new_err(
                    "a polling sensor's RunRequest needs a run_key, which makes repeated polls start the run once",
                ));
            }
        };
        runs.push(RequestedRun { job, run_key });
    }
    Ok(SensorFunctionOutcome::Requested { cursor, runs })
}

fn returned_error() -> PyErr {
    PyValueError::new_err(
        "a sensor returns None, a SkipReason, a RunRequest or a list of RunRequest records",
    )
}
