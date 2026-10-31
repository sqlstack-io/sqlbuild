//! Execute only the API's frozen request after compiling and verifying the unmodified baseline.

use std::sync::{Arc, mpsc};
use std::time::Duration;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_orchestration::runs::models::RunOutcome;

use crate::bindings::_helpers::boundary::config_values::json_value_to_python;
use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with};
use crate::bindings::_helpers::build_command::command::run_build;
use crate::bindings::_helpers::compiled_project::snapshot::NativeOrchestrationSnapshot;
use crate::bindings::_helpers::compiled_project::snapshot_execution::WorkerBuild;
use crate::bindings::_helpers::compiled_project::snapshot_worker::NativeOrchestrationWorker;
use crate::bindings::_helpers::native_host::host::{entry, write_console};
use crate::bindings::_helpers::recall::session::InvocationRecall;
use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::ids::RecordId;

fn build_request<'py>(
    py: Python<'py>,
    discovered: Bound<'py, PyAny>,
    arguments: &KeywordArguments<'py>,
    worker: &Bound<'py, NativeOrchestrationWorker>,
) -> PyResult<Bound<'py, PyAny>> {
    let execution = worker.get().execution(py)?;
    let frozen = execution.claim.request;
    for name in frozen.settings.keys() {
        if !matches!(
            name.as_str(),
            "full_refresh"
                | "fail_fast"
                | "concurrency"
                | "cli_vars"
                | "run_tests"
                | "run_audits"
                | "load_sources"
                | "reload_sources"
                | "include_python"
                | "allow_snapshot_full_refresh"
                | "allow_snapshot_schema_change"
                | "allow_table_type_downgrade"
                | "allow_retention_decrease"
                | "allow_missing_migration_origin"
                | "max_microbatches"
                | "test_timeout"
        ) {
            return Err(PyValueError::new_err(format!(
                "unsupported worker build setting {name}"
            )));
        }
    }
    let settings = PyDict::new(py);
    for (name, value) in &frozen.settings {
        settings.set_item(name, json_value_to_python(py, value)?)?;
    }
    settings.set_item("project_dir", arguments.value("project_dir")?)?;
    settings.set_item("selected_target", frozen.target)?;
    let mut selectors: Vec<String> = Vec::new();
    for key in &frozen.selection {
        let (_, name) = key
            .split_once(':')
            .ok_or_else(|| PyValueError::new_err("frozen run contains an invalid node key"))?;
        selectors.push(name.to_owned());
    }
    settings.set_item("select", selectors)?;
    settings.set_item("discovered_inputs", discovered)?;
    settings.set_item("no_color", true)?;
    settings.set_item("json_output", true)?;
    new_record(py, RecordId::BuildCommandRequest, (), Some(&settings))
}

/// The heartbeat is native; only driver cancellation crosses the executor host boundary.
fn run_cancellable(
    py: Python<'_>,
    worker: &Arc<WorkerBuild>,
    request: &Bound<'_, PyAny>,
) -> PyResult<i32> {
    let state = Arc::clone(worker);
    let (stop, stopping) = mpsc::channel::<()>();
    let thread = std::thread::Builder::new()
        .name("sqb-worker-cancel".into())
        .spawn(move || {
            while matches!(
                stopping.recv_timeout(Duration::from_millis(100)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ) {
                let _ = Python::attach(|py| -> PyResult<()> {
                    if state.worker.bind(py).get().cancel_requested(py) {
                        call_with(&entry(py, ExecutorHost::CancelRunningStatements)?, &[])?;
                    }
                    Ok(())
                });
            }
        })
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    let result = run_build(py, request, Some(Arc::clone(worker)));
    drop(stop);
    py.detach(|| thread.join())
        .map_err(|_| PyRuntimeError::new_err("worker cancellation thread failed"))?;
    result
}

pub(super) fn execute_worker<'py>(
    py: Python<'py>,
    (snapshot, discovered): (&NativeOrchestrationSnapshot, Bound<'py, PyAny>),
    arguments: &KeywordArguments<'py>,
    (bearer, identity, recall): (&str, (String, String), Option<&InvocationRecall>),
) -> PyResult<i32> {
    write_console(py, "stderr", "Verifying frozen run inputs...\n")?;
    let owner = Bound::new(
        py,
        NativeOrchestrationWorker::start(
            snapshot,
            py,
            (&arguments.value("server_url")?.extract::<String>()?, bearer),
            identity,
        )
        .map_err(|error| PyValueError::new_err(error.to_string()))?,
    )?;
    let worker = WorkerBuild::new(&owner)?;
    if let Some(recall) = recall {
        *recall
            .worker
            .lock()
            .map_err(|_| PyRuntimeError::new_err("worker completion lock is unavailable"))? =
            Some(Arc::clone(&worker));
    }
    let result = (|| {
        let request = build_request(py, discovered, arguments, &owner)?;
        write_console(
            py,
            "stderr",
            "Run inputs verified. Executing frozen selection...\n",
        )?;
        run_cancellable(py, &worker, &request)
    })();
    let code = result.as_ref().copied().unwrap_or(1);
    let outcome = if recall.is_some() {
        worker.outcome(py, code)?
    } else {
        worker.finish(py, code)?
    };
    result.map(|code| {
        if matches!(outcome, RunOutcome::Success | RunOutcome::Warn) {
            code
        } else {
            code.max(1)
        }
    })
}
