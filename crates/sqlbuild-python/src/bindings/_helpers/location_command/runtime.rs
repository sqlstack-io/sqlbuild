//! A location keeps its interpreter, and a synchronization its warehouse session, warm.

use std::io::{self, Write};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_orchestration::sensors::models::{CustomSensorOutcome, SensorEvaluationRequest};
use sqlbuild_orchestration::warehouse_metadata::models::{
    WarehouseHistoryKind, WarehouseHistoryRead,
};

use crate::bindings::_helpers::location_command::command::{location_arguments, publish_location};
use crate::bindings::_helpers::location_command::constants::{
    DUCKDB_LOCK_CONFLICT, METADATA_ERROR_CHARS, PLAN_REQUEST,
};
use crate::bindings::_helpers::location_command::runtime_plan::runtime_plan;
use crate::bindings::_helpers::location_command::types::HeldProject;
use sqlbuild_python_base::native_host::main::redirect_stdout_to_stderr::redirect_stdout_to_stderr;
use crate::bindings::_helpers::native_host::orchestration_sensors::evaluate_sensor;
use crate::bindings::_helpers::native_host::warehouse_metadata::{
    close_history_session, open_history_session, read_history,
};
use crate::bindings::_helpers::recall::console::write_stderr;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::main::redirect_stdout_to_stderr::redirect_stdout_to_stderr;

pub(super) fn run_runtime(py: Python<'_>, options: Bound<'_, PyDict>) -> PyResult<i32> {
    options.set_item("action", "publish")?;
    let redirect = redirect_stdout_to_stderr(py)?;
    within(&redirect, |_| {
        let mut session = None;
        let mut stream = None;
        let mut held: Option<HeldProject<'_>> = None;
        let mut reported_busy = false;
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
                    held = None;
                    let (published, rules) = match publish_location(py, &arguments) {
                        Ok(Ok((published, project))) => {
                            held = Some(project);
                            (Some(published), None)
                        }
                        Ok(Err(rules)) => (None, Some(rules)),
                        Err(error) => {
                            write_stderr(
                                py,
                                &format!("Location compilation/publication failed: {error}\n"),
                            )?;
                            (None, None)
                        }
                    };
                    serde_json::to_string(&serde_json::json!({
                        "published": published,
                        "rules": rules,
                    }))
                } else if let Some(plan) = request.get(PLAN_REQUEST) {
                    serde_json::to_string(&runtime_plan(py, held.as_ref(), plan)?)
                } else if WarehouseHistoryKind::ALL
                    .iter()
                    .any(|kind| request.get(kind.as_str()).is_some())
                {
                    let metadata: WarehouseHistoryRead =
                        serde_json::from_value(request).map_err(|_| {
                            PyValueError::new_err("runtime received an invalid metadata read")
                        })?;
                    let mut busy = false;
                    let mut failure: Option<String> = None;
                    if session.is_none() {
                        let arguments = location_arguments(py, Some(options.clone()))?;
                        session = match open_history_session(py, &arguments) {
                            Ok(opened) => Some(opened),
                            Err(error) => {
                                busy = error.to_string().contains(DUCKDB_LOCK_CONFLICT);
                                if !busy {
                                    failure = Some(format!(
                                        "warehouse connection for metadata reads failed: {error}"
                                    ));
                                } else if !reported_busy {
                                    write_stderr(
                                        py,
                                        "Warehouse file is in use by another process, such as a DuckDB build; the sync waits for it.\n",
                                    )?;
                                }
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
                                Err(error) => {
                                    failure = Some(error.to_string());
                                    None
                                }
                            }
                        }
                        None => None,
                    };
                    if session.as_ref().is_some_and(|opened| opened.local)
                        && let Some(opened) = session.take()
                    {
                        drop(close_history_session(opened, stream.take()));
                    }
                    reported_busy = busy;
                    if busy {
                        serde_json::to_string(&serde_json::json!({ "busy": true }))
                    } else if let Some(page) = page {
                        serde_json::to_string(&page)
                    } else {
                        if let Some(opened) = session.take() {
                            drop(close_history_session(opened, None));
                        }
                        let message = failure.unwrap_or_else(|| "no page was read".into());
                        serde_json::to_string(&serde_json::json!({
                            "error": message.chars().take(METADATA_ERROR_CHARS).collect::<String>()
                        }))
                    }
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
