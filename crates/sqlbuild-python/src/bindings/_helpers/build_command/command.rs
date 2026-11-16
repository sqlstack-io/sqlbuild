//! `sqb build`: resolve, compile, gate, execute and report one direct build.

use std::sync::Arc;
use std::time::Instant;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_cli::person_login::main::sync_after_build::sync_after_build;
use sqlbuild_host::entries::models::{AdapterHost, ExecutorHost, UserCodeHost};

use crate::bindings::_helpers::build_command::completion::{
    complete_direct_build, finalize_exceptional_cost, finalize_no_work_with_timings,
    record_pending_cost, write_partial_timings,
};
use crate::bindings::_helpers::build_command::execution::execute_direct_build;
use crate::bindings::_helpers::build_command::invocation::{BuildContext, resolve_build_context};
use crate::bindings::_helpers::build_command::planning::{
    compile_build_plan, enforce_build_plan_policies, run_defer_clone_prephase,
    write_compile_target, write_plan_text,
};
use crate::bindings::_helpers::check::check_core::relevant_check_functions;
use crate::bindings::_helpers::compiled_project::snapshot_execution::WorkerBuild;
use crate::bindings::_helpers::discovery::provider_secrets::{
    consumer_uses, provider_uses, require_used_secrets, sink_uses,
};
use crate::bindings::_helpers::graph::python_node_graph::python_node_graph;
use crate::bindings::_helpers::observations::source_capture::{
    arm_source_capture, take_source_capture,
};
use crate::bindings::_helpers::recall::console::write_stderr;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::build_command::main::frozenset::frozenset;
use sqlbuild_python_base::build_command::main::tuple::tuple;
use sqlbuild_python_base::diagnostics::models::BuildPhaseTimingTracker;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

fn run_with_providers(context: &BuildContext<'_>) -> PyResult<i32> {
    let py: Python<'_> = context.py();
    let session: Bound<'_, PyAny> = call_with(
        &host_entry(py, UserCodeHost::BuildProviderSession)?,
        &[(
            "discovered_providers",
            context.get("discovered_inputs")?.getattr("providers")?,
        )],
    )?;
    let built: PyResult<i32> = run_planned_build(context, &native_field(&session, "providers")?);
    let closed: PyResult<Bound<'_, PyAny>> = session.call_method0("close");
    let exit_code: i32 = built?;
    closed?;
    Ok(exit_code)
}

fn run_planned_build(context: &BuildContext<'_>, providers: &Bound<'_, PyAny>) -> PyResult<i32> {
    if let Some(worker) = &context.worker {
        worker.check_cancelled(context.py())?;
        if !context.get("effective_defer_clone_from")?.is_none() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "worker defer-clone requires claimed boundary writers and is not yet supported",
            ));
        }
    }
    run_defer_clone_prephase(context)?;
    let cache_scope: Bound<'_, PyAny> = call_with(
        &host_entry(context.py(), AdapterHost::RelationMetadataCacheScope)?,
        &[("adapter", context.get("adapter")?)],
    )?;
    within(&cache_scope, |_| run_cached_build(context, providers))
}

/// Plan and run the build, capturing the source columns planning describes for its observations.
fn run_cached_build(context: &BuildContext<'_>, providers: &Bound<'_, PyAny>) -> PyResult<i32> {
    arm_source_capture();
    let built: PyResult<i32> = run_captured_build(context, providers);
    let _ = take_source_capture();
    built
}

/// The post-build check functions that run if every planned node succeeds.
fn planned_check_functions<'py>(
    context: &BuildContext<'py>,
    pipeline: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    if !native_field(&context.request, "run_audits")?.extract::<bool>()? {
        return Ok(Vec::new());
    }
    let py: Python<'py> = context.py();
    let plan: Bound<'py, PyAny> = native_field(pipeline, "plan_output")?;
    let mut dependencies: Vec<Bound<'py, PyAny>> = Vec::new();
    for entry in native_field(pipeline, "python_plan_entries")?.try_iter()? {
        dependencies.push(native_field(&entry?, "name")?);
    }
    let mut completed: Vec<Bound<'py, PyAny>> = Vec::new();
    for entries in ["model_entries", "seed_entries", "source_load_entries"] {
        for entry in native_field(&plan, entries)?.try_iter()? {
            completed.push(native_field(&entry?, "name")?);
        }
    }
    for load in native_field(&plan, "source_load_entries")?.try_iter()? {
        dependencies.push(native_field(&load?, "loader")?);
    }
    let discovered: Bound<'py, PyAny> = context.get("discovered_inputs")?;
    relevant_check_functions(
        &discovered,
        &python_node_graph(&discovered)?,
        native_field(&context.request, "exclude")?,
        (&frozenset(py, &dependencies)?, &frozenset(py, &completed)?),
    )
}

/// Fail before anything runs when planned nodes, their post-build checks or sinks lack provider secrets.
fn require_plan_secrets(context: &BuildContext<'_>, pipeline: &Bound<'_, PyAny>) -> PyResult<()> {
    let discovered: Bound<'_, PyAny> = context.get("discovered_inputs")?;
    let mut uses: Vec<(String, String)> = provider_uses(
        &native_field(&native_field(pipeline, "plan_output")?, "provider_usages")?,
        None,
    )?;
    uses.extend(consumer_uses(&native_field(
        pipeline,
        "python_plan_entries",
    )?)?);
    uses.extend(consumer_uses(&tuple(
        context.py(),
        &planned_check_functions(context, pipeline)?,
    )?)?);
    uses.extend(sink_uses(&discovered)?);
    require_used_secrets(&discovered, &uses)
}

/// Plan and run the build, sharing one relation metadata cache from planning to the run's end.
fn run_captured_build(context: &BuildContext<'_>, providers: &Bound<'_, PyAny>) -> PyResult<i32> {
    let pipeline: Bound<'_, PyAny> = compile_build_plan(context)?;
    require_plan_secrets(context, &pipeline)?;
    if let Some(worker) = &context.worker {
        worker.enforce_plan_writers(&pipeline)?;
    }
    write_plan_text(context, &pipeline)?;
    enforce_build_plan_policies(context, &pipeline)?;
    write_compile_target(context, &pipeline)?;
    if finalize_no_work_with_timings(context, &pipeline)? {
        return Ok(0);
    }
    let started_at: Bound<'_, PyAny> = record_pending_cost(context, &pipeline)?;
    match execute_direct_build(context, &pipeline, providers) {
        Ok((preparation, outcome, checks)) => complete_direct_build(
            context,
            &pipeline,
            (&preparation, &outcome, &checks),
            &started_at,
        ),
        Err(error) => {
            finalize_exceptional_cost(context, &pipeline, (&started_at, &error));
            Err(error)
        }
    }
}

fn run_within_limits(
    request: &Bound<'_, PyAny>,
    tracker: &Bound<'_, PyAny>,
    started: Instant,
    worker: Option<Arc<WorkerBuild>>,
) -> PyResult<i32> {
    let py: Python<'_> = request.py();
    let mut context: BuildContext<'_> = resolve_build_context(request, tracker, started)?;
    context.worker = worker;
    let limits: Bound<'_, PyAny> = context.get("execution_limits")?;
    let deadline: Bound<'_, PyAny> = call_with(
        &host_entry(py, ExecutorHost::ExecutionDeadlineScope)?,
        &[
            (
                "max_duration_seconds",
                native_field(&limits, "max_duration_seconds")?,
            ),
            ("max_duration", native_field(&limits, "max_duration")?),
            ("target_name", context.get("effective_target_name")?),
            ("remediation", native_field(&limits, "remediation")?),
        ],
    )?;
    let exit_code: i32 = within(&deadline, |_| run_with_providers(&context))?;
    if context.worker.is_none() {
        let _ = request_location_sync(&context);
    }
    Ok(exit_code)
}

/// A signed-in person's build refreshes its configured location's warehouse copy; never fails it.
fn request_location_sync(context: &BuildContext<'_>) -> PyResult<()> {
    let py: Python<'_> = context.py();
    let target: Bound<'_, PyAny> = context.get("effective_target_name")?;
    if target.is_none() {
        return Ok(());
    }
    let target: String = target.extract()?;
    let project_dir: std::path::PathBuf = context.get("effective_project_dir")?.extract()?;
    if let Some(line) = py.detach(|| sync_after_build(&project_dir, &target)) {
        write_stderr(py, &line)?;
    }
    Ok(())
}

/// Run one `sqb build` request.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_build_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    run_build(py, request, None)
}

/// A claim owner survives planning, execution and connection cleanup before terminal reporting.
pub(crate) fn run_build(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
    worker: Option<Arc<WorkerBuild>>,
) -> PyResult<i32> {
    let tracker: Bound<'_, PyAny> = BuildPhaseTimingTracker::new(py, None)
        .and_then(|value| pyo3::Bound::new(py, value))
        .map(pyo3::Bound::into_any)?;
    let reporting: Bound<'_, PyAny> = call_with(
        &host_entry(py, ExecutorHost::ProcessResourceReporting)?,
        &[("enabled", native_field(request, "debug")?)],
    )?;
    within(&reporting, |_| {
        within(&tracker.call_method0("scope")?, |_| {
            let started: Instant = Instant::now();
            run_within_limits(request, &tracker, started, worker.clone()).inspect_err(|_| {
                let verbose: bool = native_field(request, "verbose")
                    .and_then(|value| value.is_truthy())
                    .unwrap_or(false)
                    || native_field(request, "debug")
                        .and_then(|value| value.is_truthy())
                        .unwrap_or(false);
                if verbose {
                    let _ = write_partial_timings(py, &tracker);
                }
            })
        })
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_build_command, module)?)?;
    Ok(())
}
