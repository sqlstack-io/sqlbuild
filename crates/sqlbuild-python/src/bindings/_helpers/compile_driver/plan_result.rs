//! Plan a compiled project over an open warehouse connection, and open that connection.

use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_planner::planner_warehouse::models::PlannerReads;

use sqlbuild_python_compile::compile_driver::main::stages::build_deferred_locations::build_deferred_locations;
use sqlbuild_python_compile::compile_driver::main::stages::gather_deferred_relations::gather_deferred_relations;
use sqlbuild_python_compile::compile_driver::main::stages::resolve_deferred_target_config::resolve_deferred_target_config;
use sqlbuild_python_compile::compile_driver::main::setup::build_project_graph_from_compiled_project::build_project_graph_from_compiled_project;
use sqlbuild_python_compile::compile_driver::main::setup::build_python_run_plan_outputs::build_python_run_plan_outputs;
use sqlbuild_python_compile::compile_driver::main::setup::python_read_source_names_for::python_read_source_names_for;
use crate::bindings::_helpers::compile_driver::job_node_settings::{
    PlannedJobSettings, planned_job_settings,
};
use sqlbuild_analysis::planner_scope::constants::SOURCE;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_planner::planner_entries::main::plans::build_execution_plan::build_execution_plan;
use sqlbuild_python_project::graph::main::nodes::native_key::native_key;
use sqlbuild_python_project::graph::models::KeyCache;
use sqlbuild_python_project::planner_phases::main::object::object;
use sqlbuild_python_project::resource_views::main::source_reads::source_entry::source_entry;
use std::collections::HashSet;

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};

use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::connection_failure::connection_failure;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_compile::compile_driver::main::stages::monotonic::monotonic;
use sqlbuild_python_project::graph::main::python_nodes::run_selection_from_inputs::run_selection_from_inputs;

const SINGLE_CONNECTION: i64 = 1;

/// The deferred relation locations and the warehouse relations behind them, when deferring.
type Deferral<'py> = (Bound<'py, PyAny>, Bound<'py, PyAny>);

/// The planning inputs: the discovered project, the adapter and its open connection.
pub(crate) struct PlanningInputs<'a, 'py> {
    pub(crate) discovered_inputs: &'a Bound<'py, PyAny>,
    pub(crate) adapter: &'a Bound<'py, PyAny>,
    /// The live planning connection, `None` when the plan reads a synced state.
    pub(crate) connection: &'a Bound<'py, PyAny>,
    pub(crate) reads: &'a Bound<'py, PlannerReads>,
    /// Python nodes' `(database, schema)` under the active target.
    pub(crate) node_results_location: Option<(Option<String>, Option<String>)>,
}

/// The selected Python nodes and SQL keys when loading is off: loaders out, their sources in.
fn without_loaders<'py>(
    (names, sql_keys): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (options, discovered, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = names.py();
    if native_field(options, "auto_load_sources")?.is_truthy()? {
        return Ok((names.clone(), sql_keys.clone()));
    }
    let mut loaders: HashSet<String> = HashSet::new();
    for loader in native_field(discovered, "loader_functions")?.try_iter()? {
        loaders.insert(native_field(&loader?, "name")?.extract()?);
    }
    let mut runnable: Vec<String> = Vec::new();
    let mut dropped: HashSet<String> = HashSet::new();
    for name in names.try_iter()? {
        let name: String = name?.extract()?;
        if loaders.contains(&name) {
            dropped.insert(name);
        } else {
            runnable.push(name);
        }
    }
    let mut sources: Vec<(String, String)> = Vec::new();
    for source in sqlbuild_python_project::compiled_views::main::project::sources::sources(project)?
        .try_iter()?
    {
        let source = source?;
        let loader: Option<String> = native_field(&source_entry(&source)?, "loader")?.extract()?;
        if loader.is_some_and(|loader| dropped.contains(&loader)) {
            sources.push((
                SOURCE.to_owned(),
                sqlbuild_python_project::resource_views::main::source_reads::name::name(&source)?
                    .extract()?,
            ));
        }
    }
    let mut selected: Vec<(String, String)> = sql_keys
        .try_iter()?
        .map(|key| native_key(&key?))
        .collect::<PyResult<_>>()?;
    selected.extend(sources);
    let selected_keys = PyFrozenSet::new(py, KeyCache::new(py)?.keys(&selected)?)?;
    Ok((
        PyFrozenSet::new(py, runnable)?.into_any(),
        selected_keys.into_any(),
    ))
}

/// Plan the compiled project over an open connection and attach Python node plans.
pub(crate) fn pipeline_result<'py>(
    inputs: &PlanningInputs<'_, 'py>,
    project: &Bound<'py, PyAny>,
    options: &Bound<'py, PyAny>,
    on_progress: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    let discovered: &Bound<'py, PyAny> = inputs.discovered_inputs;
    let select: Bound<'py, PyAny> = native_field(options, "select")?;
    let exclude: Bound<'py, PyAny> = native_field(options, "exclude")?;
    let (deferred_locations, deferred_relations): Deferral<'py> =
        deferral(inputs, project, &native_field(options, "defer_to")?)?;
    let empty = || -> PyResult<Bound<'py, PyAny>> { Ok(PyFrozenSet::empty(py)?.into_any()) };
    let mut selected_sql_keys: Bound<'py, PyAny> = py.None().into_bound(py);
    let mut selected_python_node_names: Bound<'py, PyAny> = empty()?;
    let mut python_read_source_names: Bound<'py, PyAny> = empty()?;
    let mut run_selection: Bound<'py, PyAny> = py.None().into_bound(py);
    let sql_test_selection: Bound<'py, PyAny> = native_field(options, "sql_test_selection")?;
    if !native_field(&sql_test_selection, "models_selected")?.is_truthy()? {
        selected_sql_keys = empty()?;
    } else if native_field(options, "resolve_python_run_selectors")?.is_truthy()? {
        let graph: Bound<'py, PyAny> = build_project_graph_from_compiled_project(
            py,
            Some(keywords(py, &[("project", project.clone())])?),
        )?;
        run_selection =
            run_selection_from_inputs((select.extract()?, exclude.extract()?), &graph, discovered)?;
        (selected_python_node_names, selected_sql_keys) = without_loaders(
            (
                &native_field(&run_selection, "python_node_names")?,
                &native_field(&run_selection, "sql_keys")?,
            ),
            (options, discovered, project),
        )?;
        python_read_source_names = python_read_source_names_for(
            py,
            Some(keywords(
                py,
                &[
                    ("discovered_inputs", discovered.clone()),
                    ("python_node_names", selected_python_node_names.clone()),
                ],
            )?),
        )?;
    }
    let selection: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::PlannerSelection,
        &[
            ("select", select),
            ("exclude", exclude),
            ("selected_keys", selected_sql_keys),
            ("python_read_source_names", python_read_source_names),
            ("sql_test_selection", sql_test_selection),
        ],
    )?;
    let job_settings: Option<PlannedJobSettings> =
        planned_job_settings((project, discovered), options)?;
    let mut override_fields: Vec<(&str, Bound<'py, PyAny>)> = option_fields(
        options,
        &[
            "cursor_overrides",
            "full_refresh",
            "reload_sources",
            "max_microbatches",
            "concurrency",
        ],
    )?;
    if let Some(job_settings) = &job_settings {
        override_fields.push((
            "full_refresh_models",
            job_settings.full_refresh_models(py)?.into_any(),
        ));
    }
    let overrides: Bound<'py, PyAny> =
        new_record_with(py, RecordId::PlannerOverrides, &override_fields)?;
    let mut deferral_fields: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("deferred_locations", deferred_locations),
        ("deferred_relations", deferred_relations),
    ];
    deferral_fields.extend(option_fields(
        options,
        &["defer_to", "defer_sources_to", "source_deferral_enabled"],
    )?);
    let deferral: Bound<'py, PyAny> =
        new_record_with(py, RecordId::DeferralInputs, &deferral_fields)?;
    let policies: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::PlannerPolicies,
        &option_fields(
            options,
            &[
                "auto_load_sources",
                "selection_diagnostics",
                "plan_sql_tests",
                "record_migration_fingerprints",
                "ignore_audit_failures",
            ],
        )?,
    )?;
    let plan_output: Bound<'py, PyAny> = build_execution_plan(
        py,
        Some(keywords(
            py,
            &[
                ("project", project.clone()),
                ("dialect", Dialect::of(inputs.adapter)?.into_any()),
                ("reads", inputs.reads.clone().into_any()),
                ("selection", selection),
                ("overrides", overrides),
                ("deferral", deferral),
                ("policies", policies),
                ("on_progress", on_progress.clone()),
                (
                    "project_config",
                    native_field(discovered, "project_config")?,
                ),
                ("local_config", native_field(discovered, "local_config")?),
                (
                    "node_results_location",
                    inputs.node_results_location.clone().into_bound_py_any(py)?,
                ),
            ],
        )?),
    )?;
    let mut job_fields: Vec<(&str, Bound<'py, PyAny>)> = Vec::new();
    let plan_output: Bound<'py, PyAny> = match &job_settings {
        Some(job_settings) => {
            let plan_output: Bound<'py, PyAny> = job_settings.apply_to_plan(
                &plan_output,
                &native_field(options, "cli_ignore_audit_failures")?,
            )?;
            let (limits, node_jobs) =
                job_settings.job_limits(&plan_output, &native_field(options, "concurrency")?)?;
            job_fields.push(("job_limits", limits.into_any()));
            job_fields.push(("node_jobs", node_jobs.into_any()));
            plan_output
        }
        None => plan_output,
    };
    let custom_materializations: Bound<'py, PyAny> =
        host_entry(py, UserCodeHost::LoadCustomMaterializations)?
            .call1((native_field(discovered, "materialization_files")?,))?;
    let python_outputs: Bound<'py, PyAny> = build_python_run_plan_outputs(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("plan_output", plan_output),
                ("run_selection", run_selection),
                ("selected_python_node_names", selected_python_node_names),
            ],
        )?),
    )?;
    let mut result_fields: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("project", project.clone()),
        ("plan_output", native_field(&python_outputs, "plan_output")?),
        ("custom_materializations", custom_materializations),
        (
            "python_node_names",
            native_field(&python_outputs, "selected_python_node_names")?,
        ),
        (
            "python_plan_entries",
            native_field(&python_outputs, "python_plan_entries")?,
        ),
    ];
    result_fields.extend(job_fields);
    new_record_with(py, RecordId::CompilePipelineResult, &result_fields)
}

/// `(name, options.name)` for each of `names`.
fn option_fields<'a, 'py>(
    options: &Bound<'py, PyAny>,
    names: &[&'a str],
) -> PyResult<Vec<(&'a str, Bound<'py, PyAny>)>> {
    names
        .iter()
        .map(|name| Ok((*name, native_field(options, name)?)))
        .collect()
}

/// The deferred locations and relations of `defer_to`, or two `None`s when not deferring.
fn deferral<'py>(
    inputs: &PlanningInputs<'_, 'py>,
    project: &Bound<'py, PyAny>,
    defer_to: &Bound<'py, PyAny>,
) -> PyResult<Deferral<'py>> {
    let py: Python<'py> = project.py();
    if defer_to.is_none() {
        return Ok((py.None().into_bound(py), py.None().into_bound(py)));
    }
    let adapter: &Bound<'py, PyAny> = inputs.adapter;
    let target_config: Bound<'py, PyAny> = resolve_deferred_target_config(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", inputs.discovered_inputs.clone()),
                ("defer_to", defer_to.clone()),
                (
                    "current_target_name",
                    native_field(project, "effective_target_name")?,
                ),
            ],
        )?),
    )?;
    let locations: Bound<'py, PyAny> = object(
        py,
        build_deferred_locations(
            py,
            Some(keywords(
                py,
                &[
                    ("project", project.clone()),
                    ("deferred_target_config", target_config),
                    ("effective_vars", native_field(project, "effective_vars")?),
                    ("default_schema", adapter.call_method0("default_schema")?),
                    (
                        "default_database",
                        adapter.call_method0("default_database")?,
                    ),
                    (
                        "render_qualified_name",
                        native_field(adapter, "render_qualified_name")?,
                    ),
                ],
            )?),
        )?,
    )?;
    let relations: Bound<'py, PyAny> = object(
        py,
        gather_deferred_relations(
            py,
            Some(keywords(
                py,
                &[
                    ("adapter", adapter.clone()),
                    ("connection", inputs.connection.clone()),
                    ("deferred_locations", locations.clone()),
                ],
            )?),
        )?,
    )?;
    Ok((locations, relations))
}

/// Open one adapter connection and report its lifecycle through `hooks`.
pub(crate) fn connection_with_hooks<'py>(
    adapter: &Bound<'py, PyAny>,
    connection_config: &Bound<'py, PyAny>,
    hooks: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = adapter.py();
    let on_start: Bound<'py, PyAny> = native_field(hooks, "on_connection_start")?;
    if !on_start.is_none() {
        on_start.call1((SINGLE_CONNECTION,))?;
    }
    let started: f64 = monotonic(py)?;
    let elapsed = |started: f64| -> PyResult<Bound<'py, PyDict>> {
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        keywords.set_item("elapsed_seconds", monotonic(py)? - started)?;
        Ok(keywords)
    };
    let connection: Bound<'py, PyAny> = match adapter.call_method1("connect", (connection_config,))
    {
        Ok(connection) => connection,
        Err(error) => {
            let on_error: Bound<'py, PyAny> = native_field(hooks, "on_connection_error")?;
            if error.is_instance_of::<PyException>(hooks.py()) && !on_error.is_none() {
                on_error.call((SINGLE_CONNECTION,), Some(&elapsed(started)?))?;
            }
            return Err(connection_failure(adapter, &error));
        }
    };
    let on_complete: Bound<'py, PyAny> = native_field(hooks, "on_connection_complete")?;
    if !on_complete.is_none() {
        on_complete.call((SINGLE_CONNECTION,), Some(&elapsed(started)?))?;
    }
    Ok(connection)
}

/// Open one adapter connection and report its lifecycle through hooks.
#[pyfunction]
#[pyo3(signature = (**options))]
fn open_connection_with_hooks<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "open_connection_with_hooks",
        options,
        (&["adapter", "connection_config", "hooks"], &[]),
    )?;
    connection_with_hooks(
        &arguments.value("adapter")?,
        &arguments.value("connection_config")?,
        &arguments.value("hooks")?,
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(open_connection_with_hooks, module)?)
}
