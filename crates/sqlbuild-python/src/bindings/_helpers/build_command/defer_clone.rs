//! `sqb build --defer-clone-from`: clone out-of-selection boundaries before build planning.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyCFunction, PyDict, PyFrozenSet, PyString, PyTuple, PyTupleMethods};
use pyo3::{IntoPyObject, IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_core::build_command::constants::EXECUTION_LIMIT_BUILD_NOTE;
use sqlbuild_core::graph_walk::main::clone_ancestors::clone_ancestors;
use sqlbuild_core::graph_walk::models::ClonePolicy;

use sqlbuild_core::build_command::errors::BuildCommandError;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::python_calls::{call_with, keywords};
use crate::bindings::_helpers::build_command::invocation::BuildContext;
use crate::bindings::_helpers::build_command::policies::build_command_error;
use crate::bindings::_helpers::build_command::policies::enforce_model_execution_limit_binding;
use crate::bindings::_helpers::clone::clone_pipeline::run_clone_pipeline_with_options;
use crate::bindings::_helpers::compile_driver::project::build_compiled_project;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_config;
use crate::bindings::_helpers::dbt_interop::reference_resolver::resolve_external_sql_reference_resolver;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_scope::retained::{ScopeData, scope_data};
use crate::bindings::_helpers::planner_scope::scope::build_planner_scope;
use crate::bindings::_helpers::planner_selection::unit_tests::split_unit_test_selectors;
use crate::bindings::_helpers::records::construct::{enum_member, new_record_with};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::replaced;
use crate::bindings::_helpers::records::ids::RecordId;

const VIEW_MATERIALIZATION: &str = "view";
const RECREATED_TYPES: [&str; 2] = ["udf", "table_fn"];
const CLONABLE_TYPES: [&str; 4] = ["model", "seed", "udf", "table_fn"];
const MODEL_TYPE: &str = "model";

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

fn user_error(py: Python<'_>, message: String, code: &str) -> PyErr {
    build_command_error(
        py,
        BuildCommandError {
            code: Some(code.to_owned()),
            message,
            help: None,
        },
    )
}

/// Functions and view models are recreated over cloned boundaries instead of cloned.
fn recreated_over_clones(py: Python<'_>, scope: &ScopeData, key: &ObjectKey) -> PyResult<bool> {
    if RECREATED_TYPES.contains(&key.0.as_str()) {
        return Ok(true);
    }
    let Some(model) = scope.model(py, &key.1) else {
        return Ok(false);
    };
    if model.is_none() {
        return Ok(false);
    }
    let materialized: Bound<'_, PyAny> = native_field(&model, "config")?
        .getattr("values")?
        .call_method1("get", ("materialized", VIEW_MATERIALIZATION))?;
    let lowered: String = materialized.str()?.call_method0("lower")?.extract()?;
    Ok(lowered == VIEW_MATERIALIZATION)
}

/// Every key the scope's traversal can visit: the selection and the upstream graph.
fn scope_keys(scope: &ScopeData) -> Vec<ObjectKey> {
    let mut keys: Vec<_> = scope.selected.iter().cloned().collect();
    for (key, upstream) in &scope.upstream {
        keys.push(key.clone());
        keys.extend(upstream.iter().cloned());
    }
    keys
}

/// Clone boundaries or recreated ancestors, without Python graph or predicate projections.
fn traversed_names(
    scope: &ScopeData,
    upstream: &HashMap<&ObjectKey, &Vec<ObjectKey>>,
    recreated: &HashSet<ObjectKey>,
    skipped_views: bool,
) -> Vec<String> {
    let mut visited: HashSet<ObjectKey> = HashSet::new();
    let mut found: HashSet<ObjectKey> = HashSet::new();
    let Ok(()) = clone_ancestors(
        scope.selected.iter().cloned().map(Ok),
        (&mut visited, &mut found),
        |node| {
            Ok(upstream
                .get(node)
                .map_or_else(Vec::new, |parents| (*parents).clone()))
        },
        ClonePolicy {
            selected: |node: &ObjectKey| Ok(scope.selected.contains(node)),
            clonable: |node: &ObjectKey| Ok(CLONABLE_TYPES.contains(&node.0.as_str())),
            view: |node: &ObjectKey| Ok(recreated.contains(node)),
            skipped_views,
        },
    );
    let mut names: Vec<_> = found.into_iter().map(|key| key.1).collect();
    names.sort();
    names
}

/// Selected models except those explicitly disabled.
fn selected_executable_model_count(py: Python<'_>, scope: &ScopeData) -> PyResult<usize> {
    let mut count: usize = 0;
    for key in &scope.selected {
        if key.0 != MODEL_TYPE {
            continue;
        }
        let model = scope
            .model(py, &key.1)
            .unwrap_or_else(|| py.None().into_bound(py));
        if !model.is_none()
            && native_field(&model, "config")?
                .getattr("values")?
                .call_method1("get", ("enabled",))?
                .is(false.into_pyobject(py)?.to_owned().into_any())
        {
            continue;
        }
        count += 1;
    }
    Ok(count)
}

/// Boundary clone selectors, rebuilt view-chain selectors and the selected model count.
fn defer_clone_selection(scope: &Bound<'_, PyAny>) -> PyResult<(Vec<String>, Vec<String>, usize)> {
    let py: Python<'_> = scope.py();
    let scope = scope_data(scope)?;
    let mut recreated: HashSet<ObjectKey> = HashSet::new();
    for key in scope_keys(&scope) {
        if recreated_over_clones(py, &scope, &key)? {
            recreated.insert(key);
        }
    }
    let upstream: HashMap<&ObjectKey, &Vec<ObjectKey>> = scope
        .upstream
        .iter()
        .map(|(key, parents)| (key, parents))
        .collect();
    Ok((
        traversed_names(&scope, &upstream, &recreated, false),
        traversed_names(&scope, &upstream, &recreated, true),
        selected_executable_model_count(py, &scope)?,
    ))
}

/// Boundary selectors, view-chain selectors and selected executable model count of a scope.
#[pyfunction]
#[pyo3(signature = (*, scope))]
fn resolve_defer_clone_selection(
    scope: &Bound<'_, PyAny>,
) -> PyResult<(Vec<String>, Vec<String>, usize)> {
    defer_clone_selection(scope)
}

fn external_resolver<'py>(
    project_dir: &Bound<'py, PyAny>,
    discovered: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    resolve_external_sql_reference_resolver(project_dir.clone(), discovered.clone())
}

/// The resolved defer-clone prephase: what to clone, from where, into which target.
struct Prephase<'py> {
    discovered: Bound<'py, PyAny>,
    adapter: Bound<'py, PyAny>,
    origin: Bound<'py, PyAny>,
    destination: Bound<'py, PyAny>,
    select: Vec<String>,
    caused_by: Bound<'py, PyAny>,
    project_dir: Bound<'py, PyAny>,
}

fn progress(on_progress: &Bound<'_, PyAny>, message: &str) -> PyResult<()> {
    if !on_progress.is_none() {
        on_progress.call1((message,))?;
    }
    Ok(())
}

fn clone_input<'py>(
    context: &BuildContext<'py>,
    prephase: &Prephase<'py>,
    (pipeline, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    on_item: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.py();
    let project: Bound<'py, PyAny> = native_field(pipeline, "destination_project")?;
    let destination_models: Bound<'py, PyAny> =
        native_field(pipeline, "destination_model_entries")?;
    let target: Bound<'py, PyAny> = resolve_target_config(
        py,
        &native_field(&prephase.discovered, "project_config")?,
        &native_field(&prephase.discovered, "local_config")?,
        &prephase.destination.clone(),
    )?;
    let mut selected: Vec<Bound<'py, PyAny>> = Vec::new();
    for entry in destination_models.try_iter()? {
        selected.push(native_field(&entry?, "name")?);
    }
    let retention: Bound<'py, PyAny> = call_with(
        &host::entry(py, ExecutorHost::BuildDestinationRetentionRequests)?,
        &[
            ("project", project.clone()),
            (
                "adapter_name",
                native_field(&prephase.adapter, "adapter_name")?,
            ),
            (
                "selected_model_names",
                PyFrozenSet::new(py, &selected)?.into_any(),
            ),
            (
                "namespace_owned",
                native_field(&target, "owns_time_travel_retention_namespace")?,
            ),
        ],
    )?;
    let allow_decrease: bool = native_field(&target, "time_travel_retention_decrease")?
        .eq(enum_member(py, RecordId::RetentionDecreasePolicy, "ALLOW")?)?;
    let sources: Bound<'py, PyAny> = call_with(
        &host::entry(py, ExecutorHost::CloneSourceEntries)?,
        &[
            ("origin", native_field(pipeline, "origin_source_entries")?),
            (
                "destination",
                native_field(pipeline, "destination_source_entries")?,
            ),
        ],
    )?;
    call_with(
        &host::entry(py, ExecutorHost::CloneExecutionInput)?,
        &[
            ("source_entries", sources),
            (
                "origin_model_entries",
                native_field(pipeline, "origin_model_entries")?,
            ),
            ("destination_model_entries", destination_models),
            (
                "origin_seed_entries",
                native_field(pipeline, "origin_seed_entries")?,
            ),
            (
                "destination_seed_entries",
                native_field(pipeline, "destination_seed_entries")?,
            ),
            (
                "destination_function_entries",
                native_field(pipeline, "destination_function_entries")?,
            ),
            (
                "execution_order",
                native_field(pipeline, "clone_plan")?.getattr("execution_order")?,
            ),
            ("adapter", prephase.adapter.clone()),
            ("destination_connection", connection.clone()),
            ("hard_copy", false.into_pyobject(py)?.to_owned().into_any()),
            ("run_id", native_field(&project, "run_id")?),
            (
                "query_change_tracking",
                native_field(&project, "settings")?.getattr("query_change_tracking")?,
            ),
            (
                "destination_transient_models",
                call_with(
                    &host::entry(py, ExecutorHost::BuildDestinationTransientModels)?,
                    &[("project", project.clone())],
                )?,
            ),
            ("destination_retention_requests", retention),
            (
                "allow_namespace_retention_decrease",
                allow_decrease.into_pyobject(py)?.to_owned().into_any(),
            ),
            ("on_item", on_item),
        ],
    )
}

/// Execute the clone, through the prephase progress stream when there is one.
fn executed_clone<'py>(
    context: &BuildContext<'py>,
    prephase: &Prephase<'py>,
    (pipeline, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.py();
    let progress_stream: Bound<'py, PyAny> = native_field(&context.invocation, "progress_stream")?;
    let execute: Bound<'py, PyAny> = host::entry(py, ExecutorHost::ExecuteClone)?;
    if progress_stream.is_none() {
        let inputs: Bound<'py, PyAny> = clone_input(
            context,
            prephase,
            (pipeline, connection),
            py.None().into_bound(py),
        )?;
        return call_with(&execute, &[("inputs", inputs)]);
    }
    let prepared: (Py<PyAny>, Py<PyAny>) = (
        clone_input(
            context,
            prephase,
            (pipeline, connection),
            py.None().into_bound(py),
        )?
        .unbind(),
        execute.unbind(),
    );
    let run_clone: Bound<'py, PyCFunction> = PyCFunction::new_closure(
        py,
        None,
        None,
        move |arguments: &Bound<'_, PyTuple>,
              _: Option<&Bound<'_, PyDict>>|
              -> PyResult<Py<PyAny>> {
            let py: Python<'_> = arguments.py();
            let inputs: Bound<'_, PyAny> =
                replaced(prepared.0.bind(py), &[("on_item", arguments.get_item(0)?)])?;
            Ok(prepared
                .1
                .bind(py)
                .call((), Some(&keywords(py, &[("inputs", inputs)])?))?
                .unbind())
        },
    )?;
    call_with(
        &host::entry(py, ExecutorHost::RunPrephaseCloneStream)?,
        &[
            ("stream", progress_stream),
            ("title", text(py, "defer clone")),
            ("caused_by_names", prephase.caused_by.clone()),
            ("use_color", native_field(&context.invocation, "use_color")?),
            ("run_clone", run_clone.into_any()),
        ],
    )
}

fn failed_items(result: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let py: Python<'_> = result.py();
    let status: Bound<'_, PyAny> = host::entry(py, ExecutorHost::CloneStatus)?;
    let (failed, warning) = (
        native_field(&status, "FAILED")?,
        native_field(&status, "WARNING")?,
    );
    let mut items: Vec<String> = Vec::new();
    for item in native_field(result, "item_results")?.try_iter()? {
        let item: Bound<'_, PyAny> = item?;
        let item_status: Bound<'_, PyAny> = native_field(&item, "status")?;
        if !(item_status.eq(&failed)? || item_status.eq(&warning)?) {
            continue;
        }
        let message: Bound<'_, PyAny> = native_field(&item, "message")?;
        let detail: Bound<'_, PyAny> = if message.is_truthy()? {
            message
        } else {
            native_field(&item, "action")?.getattr("value")?
        };
        items.push(format!(
            "{}: {}",
            native_field(&item, "name")?.str()?,
            detail.str()?
        ));
    }
    Ok(items)
}

/// Clone the boundary relations from the origin target, then copy their fingerprints.
fn clone_boundaries<'py>(
    context: &BuildContext<'py>,
    prephase: &Prephase<'py>,
    connection: &Bound<'py, PyAny>,
) -> PyResult<()> {
    let py: Python<'py> = context.py();
    let on_progress: Bound<'py, PyAny> =
        native_field(&context.invocation, "planning_progress")?.getattr("on_progress")?;
    let start: Instant = Instant::now();
    let request: &Bound<'py, PyAny> = &context.request;
    let pipeline: Bound<'py, PyAny> = run_clone_pipeline_with_options(Some(&keywords(
        py,
        &[
            ("discovered_inputs", prephase.discovered.clone()),
            ("adapter", prephase.adapter.clone()),
            ("origin_target_name", prephase.origin.clone()),
            ("destination_target_name", prephase.destination.clone()),
            (
                "destination_connection",
                new_record_with(
                    py,
                    RecordId::ClonePipelineConnection,
                    &[
                        (
                            "config",
                            native_field(&context.invocation, "connection_config")?,
                        ),
                        ("handle", connection.clone()),
                    ],
                )?,
            ),
            (
                "options",
                new_record_with(
                    py,
                    RecordId::ClonePipelineOptions,
                    &[
                        (
                            "no_sql_validation",
                            native_field(request, "no_sql_validation")?,
                        ),
                        ("no_cache", native_field(request, "no_cache")?),
                        ("select", PyTuple::new(py, &prephase.select)?.into_any()),
                        ("cli_vars", native_field(request, "cli_vars")?),
                    ],
                )?,
            ),
            (
                "external_sql_reference_resolver",
                external_resolver(&prephase.project_dir, &prephase.discovered)?,
            ),
        ],
    )?))?;
    progress(
        &on_progress,
        &format!(
            "Prepared defer clone plan. ({:.2}s)",
            start.elapsed().as_secs_f64()
        ),
    )?;
    let result: Bound<'py, PyAny> = executed_clone(context, prephase, (&pipeline, connection))?;
    let failed: Vec<String> = failed_items(&result)?;
    if !failed.is_empty() {
        return Err(user_error(
            py,
            format!(
                "failed to clone one or more deferred boundary relations: {}",
                failed.join("; ")
            ),
            "C411",
        ));
    }
    let project: Bound<'py, PyAny> = native_field(&pipeline, "destination_project")?;
    call_with(
        &host::entry(py, ExecutorHost::CopyCloneFingerprints)?,
        &[
            ("result", result),
            (
                "origin_model_entries",
                native_field(&pipeline, "origin_model_entries")?,
            ),
            (
                "destination_model_entries",
                native_field(&pipeline, "destination_model_entries")?,
            ),
            (
                "origin_seed_entries",
                native_field(&pipeline, "origin_seed_entries")?,
            ),
            (
                "destination_seed_entries",
                native_field(&pipeline, "destination_seed_entries")?,
            ),
            ("adapter", prephase.adapter.clone()),
            ("destination_connection", connection.clone()),
            ("run_id", native_field(&project, "run_id")?),
            (
                "query_change_tracking",
                native_field(&project, "settings")?.getattr("query_change_tracking")?,
            ),
        ],
    )?;
    Ok(())
}

/// Clone the selected boundary relations from the origin before build planning.
fn run_prephase(context: &BuildContext<'_>, prephase: &Prephase<'_>) -> PyResult<()> {
    let py: Python<'_> = context.py();
    if prephase.select.is_empty() {
        return Ok(());
    }
    if prephase.destination.is_none() {
        return Err(user_error(
            py,
            "--defer-clone-from requires an active target".to_owned(),
            "C409",
        ));
    }
    if prephase.origin.eq(&prephase.destination)? {
        return Err(user_error(
            py,
            format!(
                "Cannot defer-clone from the current target '{}'",
                prephase.origin.str()?
            ),
            "C410",
        ));
    }
    progress(
        &native_field(&context.invocation, "planning_progress")?.getattr("on_progress")?,
        "Preparing defer clone plan...",
    )?;
    let connection: Bound<'_, PyAny> = prephase.adapter.call_method1(
        "connect",
        (native_field(&context.invocation, "connection_config")?,),
    )?;
    let cloned: PyResult<()> = clone_boundaries(context, prephase, &connection);
    prephase.adapter.call_method1("close", (connection,))?;
    cloned
}

/// Resolve defer-clone boundaries and clone them before build planning.
pub(crate) fn run_defer_clone_boundary_prephase(
    context: &BuildContext<'_>,
    origin: Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = context.py();
    let (request, invocation) = (&context.request, &context.invocation);
    let discovered: Bound<'_, PyAny> = native_field(invocation, "discovered_inputs")?;
    let split: Bound<'_, PyAny> = split_unit_test_selectors(
        &native_field(request, "select")?,
        &native_field(request, "exclude")?,
        &discovered.clone(),
    )?;
    if !native_field(&split, "sql_test_selection")?
        .getattr("models_selected")?
        .is_truthy()?
    {
        return Ok(());
    }
    let adapter: Bound<'_, PyAny> = native_field(invocation, "adapter")?;
    let project_dir: Bound<'_, PyAny> = native_field(invocation, "effective_project_dir")?;
    let project: Bound<'_, PyAny> = build_compiled_project(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("adapter", adapter.clone()),
                ("selected_target", native_field(request, "selected_target")?),
                (
                    "no_sql_validation",
                    native_field(request, "no_sql_validation")?,
                ),
                (
                    "analysis_selection",
                    new_record_with(
                        py,
                        RecordId::CompileAnalysisSelection,
                        &[("no_cache", native_field(request, "no_cache")?)],
                    )?,
                ),
                ("cli_vars", native_field(request, "cli_vars")?),
                (
                    "external_sql_reference_resolver",
                    external_resolver(&project_dir, &discovered)?,
                ),
            ],
        )?),
    )?;
    let scope: Bound<'_, PyAny> = build_planner_scope(
        py,
        Some(keywords(
            py,
            &[
                ("project", project.clone()),
                ("select", native_field(&split, "select")?),
                ("exclude", native_field(&split, "exclude")?),
                (
                    "auto_load_sources",
                    native_field(invocation, "should_load_sources")?,
                ),
            ],
        )?),
    )?;
    let (boundary, view_chain, model_count): (Vec<String>, Vec<String>, usize) =
        defer_clone_selection(&scope)?;
    enforce_model_execution_limit_binding(
        py,
        Some(keywords(
            py,
            &[
                ("model_count", model_count.into_pyobject(py)?.into_any()),
                (
                    "target_name",
                    native_field(invocation, "effective_target_name")?,
                ),
                ("limits", native_field(invocation, "execution_limits")?),
                (
                    "refusal_note",
                    EXECUTION_LIMIT_BUILD_NOTE.into_bound_py_any(py)?,
                ),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let mut select: Vec<String> = boundary;
    select.extend(view_chain);
    run_prephase(
        context,
        &Prephase {
            discovered,
            adapter,
            origin,
            destination: native_field(&project, "effective_target_name")?,
            select,
            caused_by: native_field(&split, "select")?,
            project_dir,
        },
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_defer_clone_selection, module)?)
}
