//! Execute only the API's frozen request after compiling and verifying the unmodified baseline.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use serde_json::Value;
use sqlbuild_analysis::definition_changes::constants::PYTHON_NODE_KINDS;
use sqlbuild_cli::argv::main::command_settings_arguments::command_settings_arguments;
use sqlbuild_cli::argv::main::job_run_arguments::job_run_arguments;
use sqlbuild_cli::argv::main::job_settings_arguments::job_settings_arguments;
use sqlbuild_cli::dispatch::main::command_dispatch::command_dispatch;
use sqlbuild_cli::dispatch::models::Dispatch;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_orchestration::runs::models::{
    RunChange, RunCommand, RunKind, RunOutcome, RunRequest, RunWindow,
};
use sqlbuild_orchestration::snapshots::main::reads::node_hashes::node_hashes;
use sqlbuild_orchestration::snapshots::models::SnapshotInput;

use crate::bindings::_helpers::audit_command::command::run_audit;
use crate::bindings::_helpers::build_command::command::run_build;
use crate::bindings::_helpers::cli::argv::parse_cli_invocation;
use crate::bindings::_helpers::cli::dispatch::Invocation;
use crate::bindings::_helpers::cli::dispatch::command_facts;
use crate::bindings::_helpers::cli::dispatch::warehouse_scope;
use crate::bindings::_helpers::command_runtime::warehouses::WarehouseInputs;
use crate::bindings::_helpers::command_runtime::warehouses::configured_warehouses;
use crate::bindings::_helpers::compile_driver::rules_at_load::RulesAtLoadScope;
use crate::bindings::_helpers::compiled_project::job_settings::job_run_settings;
use crate::bindings::_helpers::cost_report::deferred::collect_deferred_cost_after_finish;
use crate::bindings::_helpers::freshness::freshness_command::run_freshness_command;
use crate::bindings::_helpers::location_command::command::compile_snapshot;
use crate::bindings::_helpers::location_command::constants::COMMAND_IMPLEMENTATIONS;
use crate::bindings::_helpers::location_command::constants::SQL_TEST_NODE_KIND;
use crate::bindings::_helpers::query_command::project::ProjectAdapter;
use crate::bindings::_helpers::seed_command::command::run_seed;
use crate::bindings::_helpers::sql_test_command::test_command::run_test;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::json_value_to_python::json_value_to_python;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry as entry;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_front::compile_attachment::main::build_effective_connection_config::build_effective_connection_config;
use sqlbuild_python_output::recall::main::write_stderr::write_stderr;
use sqlbuild_python_output::recall::models::InvocationRecall;
use sqlbuild_python_project::compiled_project::constants::SQL_TEST_KEY_PREFIX;
use sqlbuild_python_project::compiled_project::main::with_frozen_job::with_frozen_job;
use sqlbuild_python_project::compiled_views::main::project::sql_tests::sql_tests;
use sqlbuild_python_project::project_snapshot::models::NativeOrchestrationSnapshot;
use sqlbuild_python_project::project_snapshot::models::NativeOrchestrationWorker;
use sqlbuild_python_project::project_snapshot::models::WorkerBuild;
use sqlbuild_python_project::project_snapshot::types::NodeKeys;

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
            SQL_TEST_NODE_KIND => definition
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

/// Each compiled SQL test with the nodes a failure of it blocks in a build: its expected, assertion and reference models, as the scheduler's `scope_deps` names them, and the functions a direct test checks, keyed `udf:` or `table_fn:` like the snapshot.
pub(super) fn blocked_nodes(project: &Bound<'_, PyAny>) -> PyResult<NodeKeys> {
    let mut blocked: NodeKeys = BTreeMap::new();
    for test in sql_tests(project)?.try_iter()? {
        let test = test?;
        let name: String = field(&field(&test, "key")?, "name")?.extract()?;
        let models: Vec<String> = field(&test, "target_model_names")?.extract()?;
        let mut keys: Vec<String> = models
            .into_iter()
            .map(|model| format!("model:{model}"))
            .collect();
        for resource in field(&test, "tested_resources")?.try_iter()? {
            let resource = resource?;
            let kind: String = field(&resource, "kind")?.str()?.extract()?;
            let name: String = field(&resource, "name")?.extract()?;
            keys.push(format!("{kind}:{name}"));
        }
        blocked.insert(format!("{SQL_TEST_KEY_PREFIX}{name}"), keys);
    }
    Ok(blocked)
}

/// Each snapshot node's direct upstream node keys.
fn upstream_nodes(snapshot: &NativeOrchestrationSnapshot) -> BTreeMap<String, Vec<String>> {
    snapshot
        .input
        .nodes
        .iter()
        .map(|(key, node)| {
            let deps = node.definition.get("deps").and_then(Value::as_array);
            let deps = deps.into_iter().flatten().filter_map(Value::as_str);
            (key.clone(), deps.map(str::to_owned).collect())
        })
        .collect()
}

/// A job run's job, the selectors of its frozen nodes and its settings, as its build declares it.
type FrozenJob = (String, Vec<String>, BTreeMap<String, Value>);

/// The frozen command's request, warehouse scope and, for a job's build, the frozen job it names.
fn command_request<'py>(
    py: Python<'py>,
    discovered: &Bound<'py, PyAny>,
    arguments: &KeywordArguments<'py>,
    (worker, compiled): (&Bound<'py, NativeOrchestrationWorker>, &Bound<'py, PyAny>),
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>, Option<FrozenJob>)> {
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
    let job_build: Option<&String> = frozen
        .job_key
        .as_ref()
        .filter(|_| command == RunCommand::Build);
    let flags: Vec<String> = match job_build {
        Some(job) => job_run_arguments(job, &settings),
        None if command == RunCommand::Build => job_settings_arguments(&settings),
        None => command_settings_arguments(command.as_str(), &settings),
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
    let mut selectors: Vec<String> = Vec::new();
    for key in &frozen.selection {
        if execution.skipped_nodes.contains_key(key) {
            continue;
        }
        let (kind, name) = key
            .split_once(':')
            .ok_or_else(|| PyValueError::new_err("frozen run contains an invalid node key"))?;
        let selector: String = if PYTHON_NODE_KINDS.contains(&kind) {
            key.clone()
        } else if kind == SQL_TEST_NODE_KIND {
            format!("test:{name}")
        } else {
            name.to_owned()
        };
        selectors.push(selector);
    }
    if job_build.is_none() {
        for selector in &selectors {
            argv.extend([PyString::new(py, "--select"), PyString::new(py, selector)]);
        }
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
        request = replaced(
            &request,
            &[
                ("discovered_inputs", discovered.clone()),
                ("compiled_project", compiled.clone()),
            ],
        )?;
    }
    let frozen_job: Option<FrozenJob> = job_build.map(|job| (job.clone(), selectors, settings));
    Ok((request, warehouse_scope(&args)?, frozen_job))
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

/// Compile the verified project again with an ad-hoc run's variables and record its nodes' hashes on the run.
fn record_compiled_vars<'py>(
    py: Python<'py>,
    (owner, frozen): (&Bound<'py, NativeOrchestrationWorker>, &RunRequest),
    (arguments, input, project): (&KeywordArguments<'py>, &SnapshotInput, &ProjectAdapter<'py>),
) -> PyResult<()> {
    let Some(vars) = frozen
        .settings
        .get("vars")
        .filter(|_| frozen.kind == RunKind::Adhoc)
    else {
        return Ok(());
    };
    write_stderr(py, "Compiling with the run's variables...\n")?;
    let vars = json_value_to_python(py, vars)?;
    let (compiled, _, _) = compile_snapshot(py, arguments, (input, project), Some(&vars))?;
    let node_hashes: BTreeMap<String, String> = node_hashes(&compiled.input)
        .map_err(|error| PyValueError::new_err(error.to_string()))?
        .into_iter()
        .filter(|(key, _)| frozen.selection.contains(key) || frozen.write_set.contains(key))
        .collect();
    owner
        .get()
        .change(py, "compiled", RunChange::Compiled { node_hashes })
}

pub(super) fn execute_worker<'py>(
    py: Python<'py>,
    ((snapshot, compiled), project, blocked): (
        (&NativeOrchestrationSnapshot, &Bound<'py, PyAny>),
        ProjectAdapter<'py>,
        NodeKeys,
    ),
    (arguments, input): (&KeywordArguments<'py>, &SnapshotInput),
    (bearer, identity, recall): (&str, (String, String), Option<&InvocationRecall>),
) -> PyResult<i32> {
    let discovered = project.discovered.clone();
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
    let worker = WorkerBuild::new(
        &owner,
        (attached_nodes(snapshot), blocked, upstream_nodes(snapshot)),
    )?;
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
        record_compiled_vars(py, (&owner, &frozen), (arguments, input, &project))?;
        let (request, scope, frozen_job) =
            command_request(py, &discovered, arguments, (&owner, compiled))?;
        write_stderr(py, "Run inputs verified. Executing frozen selection...\n")?;
        let _rules = RulesAtLoadScope::enter();
        let directory: (PathBuf, Option<String>) = (
            arguments.value("project_dir")?.extract()?,
            Some(frozen.target.clone()),
        );
        within(&scope, |_| match frozen_job {
            Some((job, selectors, settings)) => {
                with_frozen_job(directory, (&job, selectors, settings), || {
                    run_cancellable(py, &worker, (&request, &discovered))
                })
            }
            None => run_cancellable(py, &worker, (&request, &discovered)),
        })
    })();
    let code = result.as_ref().copied().unwrap_or(1);
    let outcome = if recall.is_some() {
        worker.outcome(py, code)?
    } else {
        let outcome = worker.finish(py, code)?;
        collect_deferred_cost_after_finish(py, &worker);
        outcome
    };
    result.map(|code| {
        if matches!(outcome, RunOutcome::Success | RunOutcome::Warn) {
            code
        } else {
            code.max(1)
        }
    })
}
