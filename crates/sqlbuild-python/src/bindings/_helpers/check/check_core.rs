//! Python check planning: run-state recording, dependency relations and read-side preflight.

use sqlbuild_host::entries::models::{AdapterHost, ExecutorHost, UserCodeHost};
use sqlbuild_python_compile::compile_driver::models::TargetSettingsValues;

use pyo3::IntoPyObject;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyFrozenSetMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_compile::compile_driver::main::setup::build_python_relation_targets::build_python_relation_targets;
use sqlbuild_python_project::graph::main::python_nodes::resolve_python_check_excludes::resolve_python_check_excludes;
use sqlbuild_python_project::planner_phases::main::object::object;

use sqlbuild_core::build_command::errors::BuildCommandError;

use crate::bindings::_helpers::build_command::policies::build_command_error;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

const MISSING_RELATION_CODE: &str = "C682";
const MODEL_KIND: &str = "model";
const SEED_KIND: &str = "seed";
const SOURCE_KIND: &str = "source";
const READ_SIDE_OPTIONS: (&[&str], &[&str]) = (
    &[
        "adapter",
        "connection_config",
        "connection",
        "pipeline_result",
        "python_graph",
        "lifecycle_plan",
        "relation_targets",
    ],
    &["validation_refs", "project_relations", "providers"],
);

fn user_error(py: Python<'_>, message: String) -> PyErr {
    build_command_error(
        py,
        BuildCommandError {
            code: Some(MISSING_RELATION_CODE.to_owned()),
            message,
            help: None,
        },
    )
}

/// The loader node result a load result stands for.
fn load_result_as_python_result<'py>(
    name: &Bound<'py, PyAny>,
    result: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = name.py();
    let execution: Bound<'py, PyAny> = host_entry(py, ExecutorHost::ExecutionStatus)?;
    let node_status: Bound<'py, PyAny> = host_entry(py, UserCodeHost::PythonNodeStatus)?;
    let status: Bound<'py, PyAny> = native_field(result, "status")?;
    let mapped: Bound<'py, PyAny> = if status.eq(native_field(&execution, "SUCCESS")?)? {
        native_field(&node_status, "SUCCESS")?
    } else if status.eq(native_field(&execution, "SKIPPED")?)? {
        native_field(&node_status, "SKIPPED")?
    } else {
        native_field(&node_status, "FAILED")?
    };
    call_with(
        &host_entry(py, ExecutorHost::PythonNodeExecutionResult)?,
        &[
            ("node_name", name.clone()),
            (
                "kind",
                host_entry(py, UserCodeHost::PythonNodeKind)?.getattr("LOADER")?,
            ),
            ("status", mapped),
            ("payload", result.clone()),
            ("error_message", native_field(result, "error_message")?),
        ],
    )
}

/// Load results keyed by loader name for check dependency lookup.
pub(crate) fn load_results_by_loader_name<'py>(
    source_map: &Bound<'py, PyAny>,
    load_results: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = source_map.py();
    let loader_by_source: Bound<'py, PyDict> = PyDict::new(py);
    for item in mapping_items(source_map)?.try_iter()? {
        let (name, source): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        let loader: Bound<'py, PyAny> = native_field(&source, "loader")?;
        if !loader.is_none() {
            loader_by_source.set_item(name, loader)?;
        }
    }
    let by_loader: Bound<'py, PyDict> = PyDict::new(py);
    for result in load_results.try_iter()? {
        let result: Bound<'py, PyAny> = result?;
        if let Some(loader) = loader_by_source.get_item(native_field(&result, "source_name")?)?
            && !loader.is_none()
        {
            by_loader.set_item(loader, result)?;
        }
    }
    Ok(by_loader)
}

/// Record Python and loader results in the Python run-state.
pub(crate) fn record_python_run_state_results<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    run_state: &Bound<'py, PyAny>,
    (python_results, load_results): (&Bound<'py, PyAny>, Option<&Bound<'py, PyAny>>),
    source_map: Option<&Bound<'py, PyAny>>,
) -> PyResult<()> {
    let py: Python<'py> = discovered_inputs.py();
    let record = |result: Bound<'py, PyAny>| -> PyResult<()> {
        call_with(
            &native_field(run_state, "record_result")?,
            &[("result", result)],
        )?;
        Ok(())
    };
    let result_by_name: Bound<'py, PyDict> = PyDict::new(py);
    for result in python_results.try_iter()? {
        let result: Bound<'py, PyAny> = result?;
        result_by_name.set_item(native_field(&result, "node_name")?, result)?;
    }
    for node in native_field(discovered_inputs, "asset_functions")?.try_iter()? {
        if let Some(result) = result_by_name.get_item(native_field(&node?, "name")?)? {
            record(result)?;
        }
    }
    let by_loader: Bound<'py, PyDict> = match (source_map, load_results) {
        (Some(source_map), Some(load_results)) => {
            load_results_by_loader_name(source_map, load_results)?
        }
        (Some(source_map), None) => {
            load_results_by_loader_name(source_map, &PyTuple::empty(py).into_any())?
        }
        _ => PyDict::new(py),
    };
    for loader in native_field(discovered_inputs, "loader_functions")?.try_iter()? {
        let loader: Bound<'py, PyAny> = loader?;
        let name: Bound<'py, PyAny> = native_field(&loader, "name")?;
        let Some(result) = by_loader.get_item(&name)? else {
            continue;
        };
        record(load_result_as_python_result(&name, &result)?)?;
    }
    Ok(())
}

/// SQL relation locations available to Python check dependencies.
pub(crate) fn build_check_relation_targets<'py>(
    adapter: &Bound<'py, PyAny>,
    pipeline_result: &Bound<'py, PyAny>,
    python_graph: &Bound<'py, PyAny>,
    selected_python_names: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = adapter.py();
    let required: Bound<'py, PyAny> = call_with(
        &native_field(python_graph, "selected_sql_refs")?,
        &[("selected_names", selected_python_names)],
    )?;
    object(
        py,
        build_python_relation_targets(
            py,
            Some(keywords(
                py,
                &[
                    ("adapter", adapter.clone()),
                    ("project", native_field(pipeline_result, "project")?),
                    ("plan_output", native_field(pipeline_result, "plan_output")?),
                    ("required_refs", required),
                ],
            )?),
        )?,
    )
}

/// Non-excluded checks whose Python and SQL dependencies all ran in this build.
pub(crate) fn relevant_check_functions<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    python_graph: &Bound<'py, PyAny>,
    exclude: Bound<'py, PyAny>,
    (selected_dependency_names, completed_sql_names): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = discovered_inputs.py();
    let excluded: Bound<'py, PyAny> = object(
        py,
        resolve_python_check_excludes(python_graph, exclude.extract::<Vec<String>>()?)?,
    )?;
    let check_kind: Bound<'py, PyAny> =
        host_entry(py, UserCodeHost::PythonNodeKind)?.getattr("CHECK")?;
    let upstream_deps: Bound<'py, PyAny> = native_field(python_graph, "upstream_deps")?;
    let mut check_names: Vec<Bound<'py, PyAny>> = Vec::new();
    for node in native_field(python_graph, "nodes")?.try_iter()? {
        let node: Bound<'py, PyAny> = node?;
        let name: Bound<'py, PyAny> = native_field(&node, "name")?;
        if !native_field(&node, "kind")?.eq(&check_kind)? || excluded.contains(&name)? {
            continue;
        }
        let upstream: Vec<Bound<'py, PyAny>> =
            mapping_get_or(&upstream_deps, &name, PyTuple::empty(py))?
                .try_iter()?
                .collect::<PyResult<_>>()?;
        let mut sql_names: Vec<Bound<'py, PyAny>> = Vec::new();
        for reference in native_field(&node, "sql_deps")?.try_iter()? {
            sql_names.push(native_field(&reference?, "name")?);
        }
        if upstream.is_empty() && sql_names.is_empty() {
            continue;
        }
        let mut ready: bool = true;
        for upstream_name in &upstream {
            ready = ready && selected_dependency_names.contains(upstream_name)?;
        }
        for sql_name in &sql_names {
            ready = ready && completed_sql_names.contains(sql_name)?;
        }
        if ready {
            check_names.push(name);
        }
    }
    let check_names: Bound<'py, PyFrozenSet> = PyFrozenSet::new(py, &check_names)?;
    let mut checks: Vec<Bound<'py, PyAny>> = Vec::new();
    for check in native_field(discovered_inputs, "check_functions")?.try_iter()? {
        let check: Bound<'py, PyAny> = check?;
        if check_names.contains(native_field(&check, "name")?)? {
            checks.push(check);
        }
    }
    Ok(checks)
}

/// Inputs of the read-side preflight `sqb check` runs before the checks themselves.
pub(crate) struct ReadSideInputs<'py> {
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) connection_config: Bound<'py, PyAny>,
    pub(crate) connection: Bound<'py, PyAny>,
    pub(crate) pipeline_result: Bound<'py, PyAny>,
    pub(crate) python_graph: Bound<'py, PyAny>,
    pub(crate) lifecycle_plan: Bound<'py, PyAny>,
    pub(crate) relation_targets: Bound<'py, PyAny>,
    pub(crate) validation_refs: Bound<'py, PyAny>,
    pub(crate) project_relations: Bound<'py, PyAny>,
    pub(crate) providers: Bound<'py, PyAny>,
    pub(crate) target_config: Option<TargetSettingsValues>,
}

fn reference_kind(reference: &Bound<'_, PyAny>) -> PyResult<String> {
    native_field(reference, "kind")?.getattr("value")?.extract()
}

/// A plan source the check can look up as a table: not an expression, with a table name.
fn table_source<'py>(
    plan_output: &Bound<'py, PyAny>,
    name: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let source: Bound<'py, PyAny> =
        native_field(plan_output, "python_source_entries")?.call_method1("get", (name,))?;
    if source.is_none()
        || !native_field(&source, "expression")?.is_none()
        || native_field(&source, "table")?.is_none()
    {
        return Ok(None);
    }
    Ok(Some(source))
}

fn located<'py>(
    plan_output: &Bound<'py, PyAny>,
    reference: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let attribute: &str = match reference_kind(reference)?.as_str() {
        MODEL_KIND => "model_locations",
        SEED_KIND => "seed_locations",
        _ => return Ok(None),
    };
    let location: Bound<'py, PyAny> = mapping_get(
        &native_field(plan_output, attribute)?,
        native_field(reference, "name")?,
    )?;
    Ok((!location.is_none()).then_some(location))
}

/// `(database, schema, name)` of the relation a reference reads, when it has one.
fn lookup_location<'py>(
    plan_output: &Bound<'py, PyAny>,
    reference: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = plan_output.py();
    let parts: (Bound<'py, PyAny>, Bound<'py, PyAny>, Bound<'py, PyAny>) =
        if reference_kind(reference)? == SOURCE_KIND {
            let Some(source) = table_source(plan_output, &native_field(reference, "name")?)? else {
                return Ok(None);
            };
            (
                native_field(&source, "database")?,
                native_field(&source, "schema")?,
                native_field(&source, "table")?,
            )
        } else {
            let Some(location) = located(plan_output, reference)? else {
                return Ok(None);
            };
            (
                native_field(&location, "database")?,
                native_field(&location, "schema")?,
                native_field(&location, "name")?,
            )
        };
    Ok(Some(
        PyTuple::new(py, [parts.0, parts.1, parts.2])?.into_any(),
    ))
}

fn relation_exists(
    lookup: &Bound<'_, PyAny>,
    (database, schema, name): (Bound<'_, PyAny>, Bound<'_, PyAny>, Bound<'_, PyAny>),
) -> PyResult<bool> {
    call_with(
        &native_field(lookup, "exists")?,
        &[("database", database), ("schema", schema), ("name", name)],
    )?
    .is_truthy()
}

/// Reject a check SQL dependency whose relation does not exist yet.
fn validate_reference_exists(
    inputs: &ReadSideInputs<'_>,
    lookup: &Bound<'_, PyAny>,
    reference: &Bound<'_, PyAny>,
) -> Result<(), PyErr> {
    let py: Python<'_> = reference.py();
    let plan_output: Bound<'_, PyAny> = native_field(&inputs.pipeline_result, "plan_output")?;
    let kind: String = reference_kind(reference)?;
    let name: Bound<'_, PyAny> = native_field(reference, "name")?;
    let (exists, relation): (bool, Bound<'_, PyAny>) = match kind.as_str() {
        MODEL_KIND | SEED_KIND => {
            let Some(location) = located(&plan_output, reference)? else {
                let noun: &str = if kind == MODEL_KIND {
                    "SQL model"
                } else {
                    "seed"
                };
                return Err(user_error(
                    py,
                    format!(
                        "Python check dependency requires unknown {noun} '{}'",
                        name.str()?
                    ),
                ));
            };
            let exists: bool = relation_exists(
                lookup,
                (
                    native_field(&location, "database")?,
                    native_field(&location, "schema")?,
                    native_field(&location, "name")?,
                ),
            )?;
            let relation: Bound<'_, PyAny> = call_with(
                &host_entry(py, AdapterHost::ResolveRelationLocationQualifiedName)?,
                &[("adapter", inputs.adapter.clone()), ("location", location)],
            )?;
            (exists, relation)
        }
        SOURCE_KIND => {
            let Some(source) = table_source(&plan_output, &name)? else {
                return Ok(());
            };
            let exists: bool = relation_exists(
                lookup,
                (
                    native_field(&source, "database")?,
                    native_field(&source, "schema")?,
                    native_field(&source, "table")?,
                ),
            )?;
            (exists, inputs.relation_targets.get_item(reference)?)
        }
        _ => return Ok(()),
    };
    if exists {
        return Ok(());
    }
    Err(user_error(
        py,
        format!(
            "Python check dependency requires existing SQL relation '{}' for {kind} '{}'; run sqb build first",
            relation.str()?,
            name.str()?
        ),
    ))
}

fn sorted_references<'py>(references: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut keyed: Vec<((String, String), Bound<'py, PyAny>)> = Vec::new();
    for reference in references.try_iter()? {
        let reference: Bound<'py, PyAny> = reference?;
        let key: (String, String) = (
            reference_kind(&reference)?,
            native_field(&reference, "name")?.extract()?,
        );
        keyed.push((key, reference));
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(keyed.into_iter().map(|(_, reference)| reference).collect())
}

fn read_side_references<'py>(
    python_graph: &Bound<'py, PyAny>,
    read_side_names: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = python_graph.py();
    let nodes: Bound<'py, PyAny> = native_field(python_graph, "nodes_by_name")?;
    let mut references: Vec<Bound<'py, PyAny>> = Vec::new();
    for name in read_side_names.try_iter()? {
        for reference in native_field(&nodes.get_item(name?)?, "sql_deps")?.try_iter()? {
            references.push(reference?);
        }
    }
    Ok(PyFrozenSet::new(py, &references)?.into_any())
}

fn node_runtime<'py>(inputs: &ReadSideInputs<'py>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = inputs.adapter.py();
    let project: Bound<'py, PyAny> = native_field(&inputs.pipeline_result, "project")?;
    let (default_database, default_schema) = inputs
        .target_config
        .clone()
        .unwrap_or_default()
        .node_locations(&inputs.adapter)?;
    call_with(
        &host_entry(py, ExecutorHost::PythonNodeRuntime)?,
        &[
            ("adapter", inputs.adapter.clone()),
            ("connection_config", inputs.connection_config.clone()),
            ("connection", inputs.connection.clone()),
            ("run_id", native_field(&project, "run_id")?),
            ("target", native_field(&project, "effective_target_name")?),
            ("vars", native_field(&project, "effective_vars")?),
            ("is_reload", false.into_pyobject(py)?.to_owned().into_any()),
            (
                "target_config",
                match &inputs.target_config {
                    Some(settings) => settings.record(py)?,
                    None => py.None().into_bound(py),
                },
            ),
            ("default_database", default_database),
            ("default_schema", default_schema),
            ("relation_targets", inputs.relation_targets.clone()),
            ("project_relations", inputs.project_relations.clone()),
            ("providers", inputs.providers.clone()),
        ],
    )
}

/// Run read-side Python dependencies for checks against existing SQL relations.
pub(crate) fn run_read_side_dependencies<'py>(
    inputs: &ReadSideInputs<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = inputs.adapter.py();
    let read_side_names: Bound<'py, PyAny> =
        native_field(&inputs.lifecycle_plan, "read_side_python_node_names")?;
    let to_validate: Bound<'py, PyAny> = if inputs.validation_refs.is_none() {
        read_side_references(&inputs.python_graph, &read_side_names)?
    } else {
        inputs.validation_refs.clone()
    };
    let references: Vec<Bound<'py, PyAny>> = sorted_references(&to_validate)?;
    let plan_output: Bound<'py, PyAny> = native_field(&inputs.pipeline_result, "plan_output")?;
    let mut locations: Vec<Bound<'py, PyAny>> = Vec::new();
    for reference in &references {
        if let Some(location) = lookup_location(&plan_output, reference)? {
            locations.push(location);
        }
    }
    let lookup: Bound<'py, PyAny> = call_with(
        &host_entry(py, AdapterHost::BuildRelationLookup)?,
        &[
            ("adapter", inputs.adapter.clone()),
            ("connection", inputs.connection.clone()),
            ("locations", PyTuple::new(py, locations)?.into_any()),
        ],
    )?;
    for reference in &references {
        validate_reference_exists(inputs, &lookup, reference)?;
    }
    if !read_side_names.is_truthy()? {
        return Ok(PyTuple::empty(py).into_any());
    }
    let tracker: Bound<'py, PyAny> = call_with(
        &host_entry(py, ExecutorHost::CreateReadSidePythonExecutionTracker)?,
        &[
            ("python_graph", inputs.python_graph.clone()),
            ("selected_python_names", read_side_names),
            ("scheduled_sql_names", PyFrozenSet::empty(py)?.into_any()),
            ("runtime", node_runtime(inputs)?),
        ],
    )?;
    tracker.call_method0("dispatch_ready_python_nodes")?;
    tracker.call_method0("finalize_unrun_python_nodes")?;
    native_field(&tracker, "results")
}

/// Run read-side Python dependencies for checks against existing SQL relations.
#[pyfunction]
#[pyo3(signature = (**options))]
fn run_check_read_side_dependencies<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "run_check_read_side_dependencies",
        options,
        READ_SIDE_OPTIONS,
    )?;
    run_read_side_dependencies(&ReadSideInputs {
        adapter: arguments.value("adapter")?,
        connection_config: arguments.value("connection_config")?,
        connection: arguments.value("connection")?,
        pipeline_result: arguments.value("pipeline_result")?,
        python_graph: arguments.value("python_graph")?,
        lifecycle_plan: arguments.value("lifecycle_plan")?,
        relation_targets: arguments.value("relation_targets")?,
        validation_refs: arguments.value("validation_refs")?,
        project_relations: arguments.value("project_relations")?,
        providers: arguments.value("providers")?,
        target_config: None,
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_check_read_side_dependencies, module)?)
}
