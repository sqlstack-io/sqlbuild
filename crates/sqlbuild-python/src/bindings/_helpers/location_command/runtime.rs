//! A location keeps its interpreter, and a synchronization its warehouse session, warm.

use std::io::{self, Write};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_orchestration::sensors::models::{CustomSensorOutcome, SensorEvaluationRequest};
use sqlbuild_orchestration::warehouse_metadata::models::{
    WarehouseHistoryKind, WarehouseHistoryRead,
};

use crate::bindings::_helpers::boundary::python_calls::within;
use crate::bindings::_helpers::location_command::command::{location_arguments, publish_location};
use crate::bindings::_helpers::native_host::host::redirect_stdout_to_stderr;
use crate::bindings::_helpers::native_host::orchestration_sensors::evaluate_sensor;
use crate::bindings::_helpers::native_host::warehouse_metadata::{
    close_history_session, open_history_session, read_history,
};
use crate::bindings::_helpers::recall::console::write_stderr;

pub(super) fn run_runtime(py: Python<'_>, options: Bound<'_, PyDict>) -> PyResult<i32> {
    options.set_item("action", "publish")?;
    let redirect = redirect_stdout_to_stderr(py)?;
    within(&redirect, |_| {
        let mut session = None;
        let mut stream = None;
        let mut serve = || -> PyResult<i32> {
            loop {
                let line = py.detach(|| -> io::Result<Option<String>> {
                    let mut line = String::new();
                    Ok((io::stdin().read_line(&mut line)? != 0).then_some(line))
                })?;
                let Some(line) = line else {
                    break Ok(0);
                };
                let request: serde_json::Value = serde_json::from_str(&line).map_err(|_| {
                    PyValueError::new_err(
                        "runtime expects a JSON generation or sensor evaluation per line",
                    )
                })?;
                let output = if let Some(generation) = request.as_i64() {
                    options.set_item("generation", generation)?;
                    let arguments = location_arguments(py, Some(options.clone()))?;
                    let record = match publish_location(py, &arguments) {
                        Ok(record) => Some(record),
                        Err(error) => {
                            write_stderr(
                                py,
                                &format!("Location compilation/publication failed: {error}\n"),
                            )?;
                            None
                        }
                    };
                    serde_json::to_string(&record)
                } else if WarehouseHistoryKind::ALL
                    .iter()
                    .any(|kind| request.get(kind.as_str()).is_some())
                {
                    let metadata: WarehouseHistoryRead =
                        serde_json::from_value(request).map_err(|_| {
                            PyValueError::new_err("runtime received an invalid metadata read")
                        })?;
                    if session.is_none() {
                        let arguments = location_arguments(py, Some(options.clone()))?;
                        session = match open_history_session(py, &arguments) {
                            Ok(opened) => Some(opened),
                            Err(_) => {
                                write_stderr(
                                    py,
                                    "Warehouse connection for metadata reads failed.\n",
                                )?;
                                None
                            }
                        };
                    }
                    let page = match &session {
                        Some(opened) => {
                            match read_history(py, (opened, stream.take()), &metadata) {
                                Ok((page, open)) => {
                                    stream = open;
                                    Some(page)
                                }
                                Err(_) => None,
                            }
                        }
                        None => None,
                    };
                    if session.as_ref().is_some_and(|opened| opened.local)
                        && let Some(opened) = session.take()
                    {
                        drop(close_history_session(opened, stream.take()));
                    }
                    if page.is_none() {
                        if let Some(opened) = session.take() {
                            drop(close_history_session(opened, None));
                        }
                        write_stderr(py, "Warehouse metadata read failed. No page delivered.\n")?;
                    }
                    serde_json::to_string(&page)
                } else {
                    let evaluation: SensorEvaluationRequest = serde_json::from_value(request)
                        .map_err(|_| {
                            PyValueError::new_err("runtime received an invalid sensor evaluation")
                        })?;
                    let arguments = location_arguments(py, Some(options.clone()))?;
                    if arguments.value("location_id")?.extract::<String>()?
                        != evaluation.location_id.to_string()
                        || arguments.value("selected_target")?.extract::<String>()?
                            != evaluation.target
                    {
                        return Err(PyValueError::new_err(
                            "sensor work belongs to another location or target",
                        ));
                    }
                    let directory = arguments
                        .value("project_dir")?
                        .extract::<std::path::PathBuf>()?;
                    let outcome = match evaluate_sensor(py, &directory, &evaluation) {
                        Ok(outcome) => outcome,
                        Err(_) => {
                            write_stderr(
                                py,
                                "Custom sensor evaluation failed. No cursor or run requests committed.\n",
                            )?;
                            CustomSensorOutcome::Failed
                        }
                    };
                    serde_json::to_string(&outcome)
                };
                let output = output.map_err(|error| PyValueError::new_err(error.to_string()))?;
                py.detach(|| -> io::Result<()> {
                    let mut stdout = io::stdout().lock();
                    writeln!(stdout, "{output}")?;
                    stdout.flush()
                })?;
            }
        };
        let result = serve();
        if let Some(opened) = session.take() {
            close_history_session(opened, stream.take())?;
        }
        result
    })
}
