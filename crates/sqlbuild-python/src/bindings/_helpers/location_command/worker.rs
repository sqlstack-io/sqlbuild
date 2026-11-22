//! Execute only the API's frozen request after compiling and verifying the unmodified baseline.

use sqlbuild_core::process_environment::constants::ENV_FILE_ERROR_VARIABLE;
use sqlbuild_core::process_environment::main::environment_var_os::environment_var_os;
use sqlbuild_python_compile::provider_check::main::key_secret_blocks::key_secret_blocks;
use sqlbuild_python_compile::provider_check::main::missing_secret_failures::missing_secret_failures;
use sqlbuild_python_compile::provider_check::main::secret_block_error::secret_block_error;
use sqlbuild_python_compile::provider_check::models::SecretBlock;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyString;
use serde_json::Value;
use sqlbuild_cli::dispatch::main::command_dispatch::command_dispatch;
use sqlbuild_cli::dispatch::models::Dispatch;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_orchestration::runs::models::{
    RunChange, RunCommand, RunKind, RunOutcome, RunRequest,
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
use crate::bindings::_helpers::cost_report::deferred::collect_deferred_cost_after_finish;
use crate::bindings::_helpers::freshness::freshness_command::run_freshness_command;
use crate::bindings::_helpers::location_command::command::compile_snapshot;
use crate::bindings::_helpers::location_command::constants::COMMAND_IMPLEMENTATIONS;
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
use sqlbuild_python_output::result_output::main::results::discard_reports::discard_reports;
use sqlbuild_python_project::compiled_project::constants::SQL_TEST_KEY_PREFIX;
use sqlbuild_python_project::compiled_views::main::project::sql_tests::sql_tests;
use sqlbuild_python_project::job_settings::main::frozen_pins::frozen_pins;
use sqlbuild_python_project::job_settings::main::frozen_run_flags::frozen_run_flags;
use sqlbuild_python_project::job_settings::main::frozen_selectors::frozen_selectors;
use sqlbuild_python_project::job_settings::main::with_frozen_job::with_frozen_job;
use sqlbuild_python_project::job_settings::main::within_pinned_build::within_pinned_build;
use sqlbuild_python_project::job_settings::models::{FrozenRunFlags, PinnedBuild};
use sqlbuild_python_project::project_snapshot::main::attached_nodes::attached_nodes;
use sqlbuild_python_project::project_snapshot::models::NativeOrchestrationSnapshot;
use sqlbuild_python_project::project_snapshot::models::NativeOrchestrationWorker;
use sqlbuild_python_project::project_snapshot::models::WorkerBuild;
use sqlbuild_python_project::project_snapshot::types::NodeKeys;

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

/// The job a job run declares, or the jobs an ad-hoc build pins, for the command to run within.
type FrozenScope = (Option<FrozenJob>, Option<PinnedBuild>);

/// The frozen command's request, warehouse scope and, for a job's build, the frozen job it names or the jobs an ad-hoc build pins.
fn command_request<'py>(
    py: Python<'py>,
    discovered: &Bound<'py, PyAny>,
    arguments: &KeywordArguments<'py>,
    (worker, compiled): (&Bound<'py, NativeOrchestrationWorker>, &Bound<'py, PyAny>),
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>, FrozenScope)> {
    let execution = worker.get().execution(py)?;
    let frozen = execution.claim.request;
    let FrozenRunFlags {
        settings,
        flags,
        job_build,
    } = frozen_run_flags(&frozen, &arguments.value("project_dir")?)?;
    let command: RunCommand = frozen.command;
    let skipped = &execution.skipped_nodes;
    let pinned = frozen_pins(
        &arguments.value("project_dir")?,
        &frozen,
        (&settings, skipped),
    )?;
    let mut argv: Vec<Bound<'py, PyString>> = std::iter::once(command.as_str().to_owned())
        .chain(flags)
        .chain(pinned.iter().flat_map(|pinned| pinned.flags.clone()))
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
    let selectors: Vec<String> = frozen_selectors(&frozen, &execution.skipped_nodes)?;
    if job_build.is_none() {
        let tokens = pinned.iter().flat_map(|pinned| pinned.tokens.iter());
        for selector in selectors.iter().chain(tokens) {
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
    let frozen_job: Option<FrozenJob> = job_build.map(|job| (job, selectors, settings));
    Ok((request, warehouse_scope(&args)?, (frozen_job, pinned)))
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
        (
            attached_nodes(&snapshot.input.nodes),
            blocked,
            upstream_nodes(snapshot),
        ),
    )?;
    if let Some(recall) = recall {
        *recall
            .worker
            .lock()
            .map_err(|_| PyRuntimeError::new_err("worker completion lock is unavailable"))? =
            Some(Arc::clone(&worker));
    }
    let result = (|| {
        if let Some(message) = environment_var_os(ENV_FILE_ERROR_VARIABLE) {
            let reason: String =
                format!("Couldn't run: {}; nothing ran", message.to_string_lossy());
            worker.refuse(&reason)?;
            write_stderr(py, &format!("{reason}\n"))?;
            return Err(PyValueError::new_err(reason));
        }
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
        let project_dir: PathBuf = arguments.value("project_dir")?.extract()?;
        let selected: BTreeSet<String> = frozen.selection.iter().cloned().collect();
        let blocks: Vec<SecretBlock> =
            key_secret_blocks((&discovered, &field(compiled, "project")?), &selected)?;
        if !blocks.is_empty() {
            let (failures, reason) = missing_secret_failures(&blocks, &project_dir);
            worker.fail_missing_secrets(py, &failures, &reason)?;
            let error: PyErr = secret_block_error(py, &blocks, &project_dir);
            write_stderr(py, &format!("{}\n", error.value(py)))?;
            return Err(error);
        }
        record_compiled_vars(py, (&owner, &frozen), (arguments, input, &project))?;
        let (request, scope, frozen_job) =
            command_request(py, &discovered, arguments, (&owner, compiled))?;
        write_stderr(py, "Run inputs verified. Executing frozen selection...\n")?;
        let _rules = RulesAtLoadScope::enter();
        let _reports = discard_reports();
        let directory: (PathBuf, Option<String>) = (
            arguments.value("project_dir")?.extract()?,
            Some(frozen.target.clone()),
        );
        within(&scope, |_| match frozen_job {
            (Some((job, selectors, settings)), _) => {
                with_frozen_job(directory, (&job, selectors, settings), || {
                    run_cancellable(py, &worker, (&request, &discovered))
                })
            }
            (None, Some(pinned)) => within_pinned_build(&pinned, || {
                run_cancellable(py, &worker, (&request, &discovered))
            }),
            (None, None) => run_cancellable(py, &worker, (&request, &discovered)),
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
