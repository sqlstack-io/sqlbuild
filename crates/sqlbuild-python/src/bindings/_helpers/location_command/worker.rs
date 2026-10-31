//! Execute only the API's frozen request after compiling and verifying the unmodified baseline.

use std::sync::{Arc, mpsc};
use std::time::Duration;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_cli::argv::main::job_settings_arguments::job_settings_arguments;
use sqlbuild_cli::dispatch::main::command_dispatch::command_dispatch;
use sqlbuild_cli::dispatch::models::Dispatch;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_orchestration::runs::models::RunOutcome;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with, within};
use crate::bindings::_helpers::build_command::command::run_build;
use crate::bindings::_helpers::cli::argv::parse_cli_invocation;
use crate::bindings::_helpers::cli::dispatch::{Invocation, command_facts, warehouse_scope};
use crate::bindings::_helpers::compiled_project::snapshot::NativeOrchestrationSnapshot;
use crate::bindings::_helpers::compiled_project::snapshot_execution::WorkerBuild;
use crate::bindings::_helpers::compiled_project::snapshot_worker::NativeOrchestrationWorker;
use crate::bindings::_helpers::native_host::host::{entry, write_console};
use crate::bindings::_helpers::recall::session::InvocationRecall;
use crate::bindings::_helpers::records::fields::{field, replaced};

fn build_request<'py>(
    py: Python<'py>,
    discovered: Bound<'py, PyAny>,
    arguments: &KeywordArguments<'py>,
    worker: &Bound<'py, NativeOrchestrationWorker>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let execution = worker.get().execution(py)?;
    let frozen = execution.claim.request;
    let mut argv: Vec<Bound<'py, PyString>> = std::iter::once("build".to_owned())
        .chain(job_settings_arguments(&frozen.settings).map_err(PyValueError::new_err)?)
        .map(|argument| PyString::new(py, &argument))
        .collect();
    argv.extend([
        PyString::new(py, "--project-dir"),
        arguments.value("project_dir")?.str()?,
        PyString::new(py, "--target"),
        PyString::new(py, &frozen.target),
        PyString::new(py, "--no-color"),
        PyString::new(py, "--json"),
    ]);
    for key in &frozen.selection {
        if execution.skipped_nodes.contains_key(key) {
            continue;
        }
        let (_, name) = key
            .split_once(':')
            .ok_or_else(|| PyValueError::new_err("frozen run contains an invalid node key"))?;
        argv.extend([PyString::new(py, "--select"), PyString::new(py, name)]);
    }
    let parsed = parse_cli_invocation(py, argv, false)?;
    let args = field(&parsed, "args")?;
    if args.is_none() {
        return Err(PyValueError::new_err("invalid frozen build settings"));
    }
    let Dispatch::Run(route, None) = command_dispatch(&command_facts(&args)?) else {
        return Err(PyValueError::new_err(
            "frozen settings must dispatch a build",
        ));
    };
    let request = Invocation::new(&args)?.value(route.arguments[0].source)?;
    Ok((
        replaced(&request, &[("discovered_inputs", discovered)])?,
        warehouse_scope(&args)?,
    ))
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
        if !worker.has_work() {
            write_console(
                py,
                "stderr",
                "Run inputs verified. All selected nodes are blocked by upstream failures.\n",
            )?;
            return Ok(0);
        }
        let (request, scope) = build_request(py, discovered, arguments, &owner)?;
        write_console(
            py,
            "stderr",
            "Run inputs verified. Executing frozen selection...\n",
        )?;
        within(&scope, |_| run_cancellable(py, &worker, &request))
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
