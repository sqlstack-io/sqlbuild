//! `sqb build`: resolve, compile, gate, execute and report one direct build.

use std::sync::Arc;
use std::time::Instant;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::{AdapterHost, ExecutorHost, UserCodeHost};

use crate::bindings::_helpers::boundary::python_calls::{call_with, within};
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
use crate::bindings::_helpers::compiled_project::snapshot_execution::WorkerBuild;
use crate::bindings::_helpers::diagnostics::trackers::BuildPhaseTimingTracker;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::fields::field as native_field;

fn run_with_providers(context: &BuildContext<'_>) -> PyResult<i32> {
    let py: Python<'_> = context.py();
    let session: Bound<'_, PyAny> = call_with(
        &host::entry(py, UserCodeHost::BuildProviderSession)?,
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
        &host::entry(context.py(), AdapterHost::RelationMetadataCacheScope)?,
        &[("adapter", context.get("adapter")?)],
    )?;
    within(&cache_scope, |_| run_cached_build(context, providers))
}

/// Plan and run the build, sharing one relation metadata cache from planning to the run's end.
fn run_cached_build(context: &BuildContext<'_>, providers: &Bound<'_, PyAny>) -> PyResult<i32> {
    let pipeline: Bound<'_, PyAny> = compile_build_plan(context)?;
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
        &host::entry(py, ExecutorHost::ExecutionDeadlineScope)?,
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
    within(&deadline, |_| run_with_providers(&context))
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
        &host::entry(py, ExecutorHost::ProcessResourceReporting)?,
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
