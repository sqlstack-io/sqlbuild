//! A location keeps its interpreter, and a synchronization its warehouse session, warm.

use std::io::{self, Write};

use pyo3::PyErr;
use pyo3::exceptions::{PyMemoryError, PyValueError};
use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_core::redaction::main::redact_error_text::redact_error_text;
use sqlbuild_orchestration::dev_events::main::emit_dev_event::emit_dev_event;
use sqlbuild_orchestration::dev_events::main::generation_failed::generation_failed;
use sqlbuild_orchestration::sensors::constants::TRY_SENSOR_REQUEST;
use sqlbuild_orchestration::sensors::models::{
    SensorEvaluationRequest, SensorFunctionOutcome, SensorTrial,
};
use sqlbuild_orchestration::warehouse_metadata::constants::DYNAMIC_RESUME_REQUEST;
use sqlbuild_orchestration::warehouse_metadata::models::{
    DynamicResumeReply, WarehouseHistoryFailure, WarehouseHistoryKind, WarehouseHistoryRead,
};

use crate::bindings::_helpers::location_command::command::location_arguments;
use crate::bindings::_helpers::location_command::command::publish_location;
use crate::bindings::_helpers::location_command::constants::DUCKDB_LOCK_CONFLICT;
use crate::bindings::_helpers::location_command::constants::PLAN_REQUEST;
use crate::bindings::_helpers::location_command::runtime_plan::runtime_plan;
use crate::bindings::_helpers::location_command::runtime_resume::runtime_resume;
use crate::bindings::_helpers::location_command::types::HeldProject;
use crate::bindings::_helpers::native_host::orchestration_sensors::{SensorCall, evaluate_sensor};
use crate::bindings::_helpers::native_host::warehouse_metadata::close_history_session;
use crate::bindings::_helpers::native_host::warehouse_metadata::open_history_session;
use crate::bindings::_helpers::native_host::warehouse_metadata::read_history;
use sqlbuild_orchestration::discovered_sets::constants::{CHECK_DISCOVERED_KEY, GENERATION_KEY};
use sqlbuild_orchestration::discovered_sets::models::{
    DiscoveredSetCheck, DiscoveryPublishRequest,
};
use sqlbuild_python_base::boundary::main::error_text_fields::error_text_fields;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::main::redirect_stdout_to_stderr::redirect_stdout_to_stderr;
use sqlbuild_python_front::discovered_sets::main::checked_set::checked_set;
use sqlbuild_python_front::discovered_sets::main::discover_for_publication::discover_for_publication;
use sqlbuild_python_front::discovered_sets::main::published_sets::published_sets;
use sqlbuild_python_output::recall::main::write_stderr::write_stderr;

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
                        "runtime expects a JSON publish request, plan, metadata read, sensor evaluation or sensor trial per line",
                    )
                })?;
                let publication: Option<DiscoveryPublishRequest> = request
                    .get(GENERATION_KEY)
                    .is_some()
                    .then(|| {
                        serde_json::from_value(request.clone()).map_err(|_| {
                            PyValueError::new_err("runtime received an invalid publish request")
                        })
                    })
                    .transpose()?;
                let output = if let Some(publication) = publication {
                    let generation: i64 = publication.generation;
                    options.set_item("generation", generation)?;
                    let arguments = location_arguments(py, Some(options.clone()))?;
                    held = None;
                    discover_for_publication(&arguments, publication)?;
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
                            let (message, code, help) = error_text_fields(py, &error)?;
                            let message = message.unwrap_or_else(|| error.to_string());
                            let failed = generation_failed(generation, message, (code, help));
                            py.detach(|| emit_dev_event(&failed))?;
                            (None, None)
                        }
                    };
                    let (discovered, discover_files) =
                        published_sets(held.as_ref().map(|(discovered, _)| discovered))?;
                    serde_json::to_string(&serde_json::json!({
                        "published": published,
                        "rules": rules,
                        "discovered": discovered,
                        "discover_files": discover_files,
                    }))
                } else if request.get(CHECK_DISCOVERED_KEY).is_some() {
                    let check: DiscoveredSetCheck = serde_json::from_value(request.clone())
                        .map_err(|_| {
                            PyValueError::new_err(
                                "runtime received an invalid discovered set check",
                            )
                        })?;
                    let arguments = location_arguments(py, Some(options.clone()))?;
                    let held_discovered = held.as_ref().map(|(discovered, _)| discovered);
                    serde_json::to_string(&checked_set(&arguments, held_discovered, check)?)
                } else if let Some(plan) = request.get(PLAN_REQUEST) {
                    serde_json::to_string(&runtime_plan(py, held.as_ref(), plan)?)
                } else if let Some(locations) = request.get(DYNAMIC_RESUME_REQUEST) {
                    serde_json::to_string(&match runtime_resume(py, &options, locations) {
                        Ok(resumed) => DynamicResumeReply::Resumed(resumed),
                        Err(error) => {
                            let reason = failure_reason(&error);
                            write_stderr(
                                py,
                                &format!("Resuming expired dynamic holds failed: {reason}\n"),
                            )?;
                            DynamicResumeReply::Failed(reason)
                        }
                    })
                } else if let Some(kind) = WarehouseHistoryKind::ALL
                    .iter()
                    .find(|kind| request.get(kind.as_str()).is_some())
                {
                    let metadata: WarehouseHistoryRead =
                        serde_json::from_value(request).map_err(|_| {
                            PyValueError::new_err("runtime received an invalid metadata read")
                        })?;
                    let mut busy = false;
                    let mut unconnected = WarehouseHistoryFailure::default();
                    if session.is_none() {
                        let arguments = location_arguments(py, Some(options.clone()))?;
                        session = match open_history_session(py, &arguments) {
                            Ok(opened) => Some(opened),
                            Err(error) => {
                                busy = error.to_string().contains(DUCKDB_LOCK_CONFLICT);
                                if !busy {
                                    unconnected.reason = failure_reason(&error);
                                    write_stderr(
                                        py,
                                        &format!(
                                            "Warehouse connection for metadata reads failed: {}\n",
                                            unconnected.reason
                                        ),
                                    )?;
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
                        Some(opened) => read_history(py, (opened, stream.take()), &metadata)
                            .map(|(page, open)| {
                                stream = open;
                                page
                            })
                            .map_err(|error| WarehouseHistoryFailure {
                                reason: failure_reason(&error),
                                page_size: error.is_instance_of::<PyMemoryError>(py),
                            }),
                        None => Err(unconnected),
                    };
                    if session.as_ref().is_some_and(|opened| opened.local)
                        && let Some(opened) = session.take()
                    {
                        drop(close_history_session(opened, stream.take()));
                    }
                    reported_busy = busy;
                    if busy {
                        serde_json::to_string(&serde_json::json!({ "busy": true }))
                    } else {
                        match page {
                            Ok(page) => serde_json::to_string(&page),
                            Err(failure) => {
                                if let Some(opened) = session.take() {
                                    drop(close_history_session(opened, None));
                                }
                                write_stderr(
                                    py,
                                    &format!(
                                        "Warehouse metadata read ({}) failed: {}. No page delivered.\n",
                                        kind.as_str(),
                                        failure.reason
                                    ),
                                )?;
                                serde_json::to_string(&serde_json::json!({ "failed": failure }))
                            }
                        }
                    }
                } else if let Some(trial) = request.get(TRY_SENSOR_REQUEST) {
                    let trial: SensorTrial =
                        serde_json::from_value(trial.clone()).map_err(|_| {
                            PyValueError::new_err("runtime received an invalid sensor trial")
                        })?;
                    let arguments = location_arguments(py, Some(options.clone()))?;
                    let directory = arguments
                        .value("project_dir")?
                        .extract::<std::path::PathBuf>()?;
                    let outcome = evaluate_sensor(
                        py,
                        &directory,
                        SensorCall {
                            sensor: &trial.sensor,
                            target: &trial.target,
                            evaluation_id: "trial",
                            event: trial.event.as_ref(),
                            cursor: None,
                            code_hash: None,
                        },
                    );
                    serde_json::to_string(&outcome)
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
                    let evaluation_id = evaluation.evaluation_id.to_string();
                    let outcome = evaluate_sensor(
                        py,
                        &directory,
                        SensorCall {
                            sensor: &evaluation.sensor,
                            target: &evaluation.target,
                            evaluation_id: &evaluation_id,
                            event: evaluation.event.as_ref(),
                            cursor: evaluation.function_cursor(),
                            code_hash: Some(&evaluation.definition.code_hash),
                        },
                    );
                    if let SensorFunctionOutcome::Failed { .. } = outcome {
                        write_stderr(
                            py,
                            &format!(
                                "Sensor {} failed. No cursor or run requests committed.\n",
                                evaluation.sensor
                            ),
                        )?;
                    }
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

/// The driver's class and message on one line, redacted as warehouse error text.
fn failure_reason(error: &PyErr) -> String {
    redact_error_text(&error.to_string())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
