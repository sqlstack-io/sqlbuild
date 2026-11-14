//! Execute only the API's frozen request after compiling and verifying the unmodified baseline.

use std::collections::BTreeMap;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use serde_json::Value;
use sqlbuild_analysis::definition_changes::constants::PYTHON_NODE_KINDS;
use sqlbuild_cli::argv::main::command_settings_arguments::command_settings_arguments;
use sqlbuild_cli::argv::main::job_settings_arguments::job_settings_arguments;
use sqlbuild_cli::dispatch::main::command_dispatch::command_dispatch;
use sqlbuild_cli::dispatch::models::Dispatch;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_orchestration::runs::models::{
    RunCommand, RunKind, RunOutcome, RunRequest, RunWindow,
};

use crate::bindings::_helpers::audit_command::command::run_audit;
use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with, within};
use crate::bindings::_helpers::build_command::command::run_build;
use crate::bindings::_helpers::cli::argv::parse_cli_invocation;
use crate::bindings::_helpers::cli::dispatch::{Invocation, command_facts, warehouse_scope};
use crate::bindings::_helpers::command_runtime::warehouses::{
    WarehouseInputs, configured_warehouses,
};
use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_connection_config;
use crate::bindings::_helpers::compiled_project::snapshot::NativeOrchestrationSnapshot;
use crate::bindings::_helpers::compiled_project::snapshot_execution::WorkerBuild;
use crate::bindings::_helpers::compiled_project::snapshot_worker::NativeOrchestrationWorker;
use crate::bindings::_helpers::discovery::project_config::configured_changes;
use crate::bindings::_helpers::freshness::freshness_command::run_freshness_command;
use crate::bindings::_helpers::location_command::constants::{
    CHANGED_SETTING, COMMAND_IMPLEMENTATIONS, JOB_RUN_COUNTS, JOB_RUN_REACH, REACH_SETTING,
};
use crate::bindings::_helpers::native_host::host::entry;
use crate::bindings::_helpers::recall::console::write_stderr;
use crate::bindings::_helpers::recall::session::InvocationRecall;
use crate::bindings::_helpers::records::fields::{field, replaced};
use crate::bindings::_helpers::seed_command::command::run_seed;
use crate::bindings::_helpers::sql_test_command::test_command::run_test;

/// Each audit and SQL test the snapshot publishes, with the node keys it checks.
fn attached_nodes(snapshot: &NativeOrchestrationSnapshot) -> BTreeMap<String, Vec<String>> {
    let mut attached: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (key, node) in &snapshot.input.nodes {
        let definition: &Value = &node.definition;
        let nodes: Vec<String> = match node.kind.as_str() {
            "audit" => definition
                .get("target")
                .and_then(|target| {
                    Some(format!(
                        "{}:{}",
                        target.get("kind")?.as_str()?,
                        target.get("name")?.as_str()?
                    ))
                })
                .into_iter()
                .collect(),
            "sql_test" => definition
                .pointer("/test/payload/expected_model_names")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|name| format!("model:{name}"))
                .collect(),
            _ => continue,
        };
        attached.insert(key.clone(), nodes);
    }
    attached
}

/// A job run's `--changed` settings: its whole selection, and source data counted unless configured.
fn job_run_settings(
    project_dir: &Bound<'_, PyAny>,
    mut settings: BTreeMap<String, Value>,
) -> PyResult<BTreeMap<String, Value>> {
    let Some(changed) = settings.get(CHANGED_SETTING).cloned() else {
        return Ok(settings);
    };
    if changed == Value::Bool(false) {
        return Ok(settings);
    }
    settings.insert(REACH_SETTING.to_owned(), Value::from(JOB_RUN_REACH));
    if changed == Value::Bool(true) && configured_changes(project_dir)?.counts.is_none() {
        settings.insert(
            CHANGED_SETTING.to_owned(),
            Value::from(JOB_RUN_COUNTS.to_vec()),
        );
    }
    Ok(settings)
}

/// The frozen command's request and warehouse scope; build carries the verified inputs in its request.
fn command_request<'py>(
    py: Python<'py>,
    discovered: &Bound<'py, PyAny>,
    arguments: &KeywordArguments<'py>,
    worker: &Bound<'py, NativeOrchestrationWorker>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let execution = worker.get().execution(py)?;
    let frozen = execution.claim.request;
    let mut settings = frozen.settings;
    if let Some(window) = frozen.window {
        for key in [
            "start_cursor",
            "end_cursor",
            "start_cursor_ts",
            "end_cursor_ts",
            "start_cursor_int",
            "end_cursor_int",
        ] {
            settings.remove(key);
        }
        let (kind, start, end) = match window {
            RunWindow::Timestamp { start, end } => ("ts", start.to_rfc3339(), end.to_rfc3339()),
            RunWindow::Integer { start, end } => ("int", start.to_string(), end.to_string()),
        };
        settings.insert(format!("start_cursor_{kind}"), serde_json::json!(start));
        settings.insert(format!("end_cursor_{kind}"), serde_json::json!(end));
    }
    let command: RunCommand = frozen.command;
    if frozen.job_key.is_some() {
        settings = job_run_settings(&arguments.value("project_dir")?, settings)?;
    }
    let flags: Vec<String> = if command == RunCommand::Build {
        job_settings_arguments(&settings)
    } else {
        command_settings_arguments(command.as_str(), &settings)
    }
    .map_err(PyValueError::new_err)?;
    let mut argv: Vec<Bound<'py, PyString>> = std::iter::once(command.as_str().to_owned())
        .chain(flags)
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
        let (kind, name) = key
            .split_once(':')
            .ok_or_else(|| PyValueError::new_err("frozen run contains an invalid node key"))?;
        let selector = if PYTHON_NODE_KINDS.contains(&kind) {
            key.as_str()
        } else {
            name
        };
        argv.extend([PyString::new(py, "--select"), PyString::new(py, selector)]);
    }
    let parsed = parse_cli_invocation(py, argv, false)?;
    let args = field(&parsed, "args")?;
    if args.is_none() {
        return Err(PyValueError::new_err("invalid frozen command settings"));
    }
    let Dispatch::Run(route, None) = command_dispatch(&command_facts(&args)?) else {
        return Err(PyValueError::new_err(
            "frozen settings must dispatch the frozen command",
        ));
    };
    if !COMMAND_IMPLEMENTATIONS.contains(&(command, route.implementation)) {
        return Err(PyValueError::new_err(
            "frozen settings must dispatch the frozen command",
        ));
    }
    let mut request = Invocation::new(&args)?.value(route.arguments[0].source)?;
    if command == RunCommand::Build {
        request = replaced(&request, &[("discovered_inputs", discovered.clone())])?;
    }
    Ok((request, warehouse_scope(&args)?))
}

/// Run the frozen command from the verified inputs, reporting its progress and outcomes to `worker`.
fn run_command<'py>(
    py: Python<'py>,
    worker: &Arc<WorkerBuild>,
    (request, discovered): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<i32> {
    let verified = Some((Arc::clone(worker), discovered));
    let code = match worker.command() {
        RunCommand::Build => run_build(py, request, Some(Arc::clone(worker))),
        RunCommand::Test => run_test(py, request, verified),
        RunCommand::Seed => run_seed(py, request, verified),
        RunCommand::Audit => run_audit(py, request, verified),
        RunCommand::Freshness => run_freshness_command(py, request, verified),
    }?;
    worker.command_returned();
    Ok(code)
}

/// Why an ad-hoc run's `--warehouse` is refused; published jobs keep their authored warehouse.
fn unconfigured_warehouse(
    discovered: &Bound<'_, PyAny>,
    frozen: &RunRequest,
) -> PyResult<Option<&'static str>> {
    if frozen.kind != RunKind::Adhoc {
        return Ok(None);
    }
    let Some(warehouse) = frozen.settings.get("warehouse") else {
        return Ok(None);
    };
    let warehouse: &str = warehouse
        .as_str()
        .ok_or_else(|| PyValueError::new_err("frozen run warehouse must be text"))?;
    let selected_target: Bound<'_, PyAny> =
        PyString::new(discovered.py(), &frozen.target).into_any();
    let connection: Bound<'_, PyAny> =
        build_effective_connection_config(discovered, Some(selected_target.clone()), None, false)?;
    let inputs = WarehouseInputs {
        discovered_inputs: discovered,
        selected_target: &selected_target,
    };
    if configured_warehouses(inputs, &connection)?.contains(warehouse) {
        return Ok(None);
    }
    Ok(Some(
        "the requested warehouse is not configured for this target on this location; \
         no warehouse statement ran",
    ))
}

/// The heartbeat is native; only driver cancellation crosses the executor host boundary.
fn run_cancellable<'py>(
    py: Python<'py>,
    worker: &Arc<WorkerBuild>,
    command: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
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
    let result = run_command(py, worker, command);
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
    write_stderr(py, "Verifying frozen run inputs...\n")?;
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
    let worker = WorkerBuild::new(&owner, attached_nodes(snapshot))?;
    if let Some(recall) = recall {
        *recall
            .worker
            .lock()
            .map_err(|_| PyRuntimeError::new_err("worker completion lock is unavailable"))? =
            Some(Arc::clone(&worker));
    }
    let result = (|| {
        if !worker.has_work() {
            write_stderr(
                py,
                "Run inputs verified. All selected nodes are blocked by upstream failures.\n",
            )?;
            return Ok(0);
        }
        let frozen = owner.get().execution(py)?.claim.request;
        if let Some(reason) = unconfigured_warehouse(&discovered, &frozen)? {
            worker.refuse(reason)?;
            write_stderr(py, &format!("Run refused: {reason}\n"))?;
            return Err(PyValueError::new_err(reason));
        }
        let (request, scope) = command_request(py, &discovered, arguments, &owner)?;
        write_stderr(py, "Run inputs verified. Executing frozen selection...\n")?;
        within(&scope, |_| {
            run_cancellable(py, &worker, (&request, &discovered))
        })
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
