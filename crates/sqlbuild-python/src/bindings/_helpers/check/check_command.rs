//! `sqb check`: run the selected Python checks after their Python dependencies.

use sqlbuild_host::entries::models::{ExecutorHost, UserCodeHost};

use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::build_progress::progress::current_native_progress_projector;
use crate::bindings::_helpers::build_progress::progress_text::write_execution_header;
use crate::bindings::_helpers::compile_driver::pipeline_entries::build_static_pipeline_result;
use crate::bindings::_helpers::compile_driver::relation_targets::build_project_relation_targets;
use crate::bindings::_helpers::compile_driver::relation_targets::build_python_relation_targets;
use crate::bindings::_helpers::compile_driver::static_context::compile_static_command_context;
use crate::bindings::_helpers::dbt_interop::reference_resolver::resolve_external_sql_reference_resolver;
use crate::bindings::_helpers::graph::algorithms::transitive_closure_many;
use crate::bindings::_helpers::graph::execution_gates::sql_ref_key;
use crate::bindings::_helpers::graph::python_lifecycle::build_python_sql_run_lifecycle;
use crate::bindings::_helpers::native_host::connections::connection_failure;
use crate::bindings::_helpers::planner_entries::command_plans::build_relation_command_plan;
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_selection::selectors::parse_project_selector;
use crate::bindings::_helpers::planner_selection::unit_tests::reject_unit_test_selectors;
use crate::bindings::_helpers::records::construct::{enum_member, new_record_with, record_class};
use crate::bindings::_helpers::result_output::documents::write_execution_json_output;
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFrozenSet, PyInt, PyString, PyTuple};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_core::check_command::errors::CheckSelectionError;
use sqlbuild_core::check_command::main::check_exit_code::check_exit_code;
use sqlbuild_core::check_command::main::render_check_summary::render_check_summary;
use sqlbuild_core::check_command::main::validate_check_selection::validate_check_selection;
use sqlbuild_core::check_command::models::CheckResultText;
use sqlbuild_core::cli_text::models::CliStyle;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::check::check_core::{
    ReadSideInputs, load_results_by_loader_name, record_python_run_state_results,
    run_read_side_dependencies,
};
use crate::bindings::_helpers::check::check_results::{
    check_result_text, results_json, write_results,
};
use crate::bindings::_helpers::command_runtime::invocation::resolve_reported_command_invocation;
use crate::bindings::_helpers::compile_outputs::runtime_target::write_python_check_runtime_target;
use crate::bindings::_helpers::graph::python_node_graph::python_node_graph;
use crate::bindings::_helpers::graph::python_selection::python_node_names;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use crate::bindings::_helpers::result_output::event_writer::ExecutionEventWriter;

const GRAPH_OPERATOR_CODE: &str = "C681";
const GRAPH_OPERATOR_MESSAGE: &str = "sqb check selectors do not support graph operators; \
    select checks directly without '+', '~', or path-between expansion";
const EXECUTION_COMMAND: &str = "sqb check";

/// The compiled pipeline and everything checks run with.
struct Prepared<'py> {
    pipeline: Bound<'py, PyAny>,
    python_graph: Bound<'py, PyAny>,
    check_functions: Bound<'py, PyAny>,
    lifecycle_plan: Bound<'py, PyAny>,
    relation_targets: Bound<'py, PyAny>,
    relation_refs: Bound<'py, PyAny>,
    defaults: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    project_relations: Bound<'py, PyAny>,
}

fn user_error(py: Python<'_>, (code, message): (&str, &str)) -> PyErr {
    let raised = (|| {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("code", code)?;
        error_class(py, ErrorId::CliUser)?.call((message,), Some(&options))
    })();
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

fn selection_error(py: Python<'_>, error: &CheckSelectionError) -> PyErr {
    user_error(py, (error.code, &error.message))
}

fn empty_tuple(py: Python<'_>) -> Bound<'_, PyAny> {
    PyTuple::empty(py).into_any()
}

/// Refuse `+`, `~` and path selectors, splitting selectors as Python's `split` does.
fn refuse_graph_selectors(py: Python<'_>, selectors: &[Bound<'_, PyAny>]) -> PyResult<()> {
    let path_selector: Bound<'_, PyAny> = record_class(py, RecordId::PathSelector)?;
    for selector in selectors {
        for token in selector.call_method0("split")?.try_iter()? {
            for part in token?.call_method1("split", (",",))?.try_iter()? {
                let parsed: Bound<'_, PyAny> =
                    parse_project_selector(part?.downcast::<PyString>()?)?;
                if parsed.is_instance(&path_selector)?
                    || native_field(&parsed, "upstream")?.is_truthy()?
                    || native_field(&parsed, "downstream")?.is_truthy()?
                {
                    return Err(user_error(
                        py,
                        (GRAPH_OPERATOR_CODE, GRAPH_OPERATOR_MESSAGE),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn python_nodes<'py>(
    graph: &Bound<'py, PyAny>,
    (select, exclude): (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    Ok(python_node_names(graph, (select.extract()?, exclude.extract()?))?.into_any())
}

fn node_kind<'py>(
    graph: &Bound<'py, PyAny>,
    name: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    native_field(graph, "nodes_by_name")?
        .get_item(name)?
        .getattr("kind")
}

/// The selected check names: selected checks, or every check minus excluded nodes.
fn selected_check_names<'py>(
    request: &Bound<'py, PyAny>,
    graph: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = graph.py();
    let select: Bound<'py, PyAny> = native_field(request, "select")?;
    let exclude: Bound<'py, PyAny> = native_field(request, "exclude")?;
    let selectors: Vec<Bound<'py, PyAny>> = select
        .try_iter()?
        .chain(exclude.try_iter()?)
        .collect::<PyResult<_>>()?;
    refuse_graph_selectors(py, &selectors)?;
    let check_kind: Bound<'py, PyAny> =
        host::entry(py, UserCodeHost::PythonNodeKind)?.getattr("CHECK")?;
    if select.is_truthy()? {
        let selected: Bound<'py, PyAny> = python_nodes(graph, (select, exclude))?;
        let mut non_checks: Vec<String> = Vec::new();
        for name in selected.try_iter()? {
            let name: Bound<'py, PyAny> = name?;
            if !node_kind(graph, &name)?.eq(&check_kind)? {
                non_checks.push(name.extract()?);
            }
        }
        validate_check_selection(&non_checks).map_err(|error| selection_error(py, &error))?;
        return selected.downcast_into().map_err(Into::into);
    }
    let excluded: Bound<'py, PyAny> = if exclude.is_truthy()? {
        python_nodes(graph, (exclude, empty_tuple(py)))?
    } else {
        PyFrozenSet::empty(py)?.into_any()
    };
    let mut names: Vec<Bound<'py, PyAny>> = Vec::new();
    for node in native_field(graph, "nodes")?.try_iter()? {
        let node: Bound<'py, PyAny> = node?;
        let name: Bound<'py, PyAny> = native_field(&node, "name")?;
        if native_field(&node, "kind")?.eq(&check_kind)? && !excluded.contains(&name)? {
            names.push(name);
        }
    }
    PyFrozenSet::new(py, names)
}

/// Compile the SQL relations and loader sources the selected checks depend on.
fn compile_pipeline<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Bound<'py, PyAny>,
    (graph, check_names): (&Bound<'py, PyAny>, &Bound<'py, PyFrozenSet>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let discovered: Bound<'py, PyAny> = native_field(invocation, "discovered_inputs")?;
    let required_refs: Bound<'py, PyAny> = call_with(
        &native_field(graph, "selected_sql_refs")?,
        &[("selected_names", check_names.clone().into_any())],
    )?;
    let closure: Bound<'py, PyAny> = object(
        py,
        transitive_closure_many(
            check_names.as_any(),
            &native_field(graph, "upstream_deps")?,
            false,
            None,
        )?,
    )?;
    let loader_kind: Bound<'py, PyAny> =
        host::entry(py, UserCodeHost::PythonNodeKind)?.getattr("LOADER")?;
    let mut loaders: Vec<Bound<'py, PyAny>> = Vec::new();
    for name in closure.try_iter()? {
        let name: Bound<'py, PyAny> = name?;
        if !check_names.contains(&name)? && node_kind(graph, &name)?.eq(&loader_kind)? {
            loaders.push(name);
        }
    }
    let loaders: Bound<'py, PyFrozenSet> = PyFrozenSet::new(py, loaders)?;
    let source_type: Bound<'py, PyAny> = enum_member(py, RecordId::CompiledResourceType, "SOURCE")?;
    let mut keys: Vec<Bound<'py, PyAny>> = Vec::new();
    for reference in required_refs.try_iter()? {
        keys.push(sql_ref_key(py, &reference?)?);
    }
    for source_file in native_field(&discovered, "source_files")?.try_iter()? {
        for source in native_field(&source_file?, "source_entries")?.try_iter()? {
            let source: Bound<'py, PyAny> = source?;
            if loaders.contains(native_field(&source, "loader")?)? {
                keys.push(new_record_with(
                    py,
                    RecordId::CompiledObjectKey,
                    &[
                        ("resource_type", source_type.clone()),
                        ("name", native_field(&source, "name")?),
                    ],
                )?);
            }
        }
    }
    let keys: Bound<'py, PyAny> = PyFrozenSet::new(py, keys)?.into_any();
    let resolver: Bound<'py, PyAny> = resolve_external_sql_reference_resolver(
        native_field(invocation, "effective_project_dir")?,
        discovered.clone(),
    )?;
    let options: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::CompilePipelineOptions,
        &[
            ("selected_target", native_field(request, "selected_target")?),
            (
                "no_sql_validation",
                native_field(request, "no_sql_validation")?,
            ),
            (
                "connection_config",
                native_field(invocation, "connection_config")?,
            ),
            ("cli_vars", native_field(request, "cli_vars")?),
            ("external_sql_reference_resolver", resolver),
        ],
    )?;
    let context: Bound<'py, PyAny> = compile_static_command_context(Some(&keywords(
        py,
        &[
            ("discovered_inputs", discovered),
            ("adapter", native_field(invocation, "adapter")?),
            ("options", options),
            ("selected_keys", keys.clone()),
            ("relation_keys", keys),
            (
                "on_progress",
                native_field(invocation, "planning_progress")?.getattr("on_progress")?,
            ),
        ],
    )?))?;
    let plan_output: Bound<'py, PyAny> = build_relation_command_plan(
        &native_field(&context, "project")?,
        &native_field(&context, "scope")?,
        &native_field(&context, "relations")?,
    )?;
    build_static_pipeline_result(
        py,
        Some(keywords(
            py,
            &[("context", context), ("plan_output", plan_output)],
        )?),
    )
}

fn target_default<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    (field, fallback): (&str, &str),
) -> PyResult<Bound<'py, PyAny>> {
    let value: Bound<'py, PyAny> = native_field(project, field)?;
    if value.is_none() {
        adapter.call_method0(fallback)
    } else {
        Ok(value)
    }
}

/// The discovered Python graph, after refusing unit-test selectors, and the selected checks.
fn python_selection<'py>(
    request: &Bound<'py, PyAny>,
    discovered: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyFrozenSet>)> {
    let _py: Python<'py> = request.py();
    let python_graph: Bound<'py, PyAny> = python_node_graph(discovered)?;
    reject_unit_test_selectors(
        &native_field(request, "select")?,
        &native_field(request, "exclude")?,
        discovered,
    )?;
    let check_names: Bound<'py, PyFrozenSet> = selected_check_names(request, &python_graph)?;
    Ok((python_graph, check_names))
}

fn prepare<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Bound<'py, PyAny>,
) -> PyResult<Prepared<'py>> {
    let py: Python<'py> = request.py();
    let discovered: Bound<'py, PyAny> = native_field(invocation, "discovered_inputs")?;
    let adapter: Bound<'py, PyAny> = native_field(invocation, "adapter")?;
    let (planning_graph, planning_names) = python_selection(request, &discovered)?;
    let pipeline: Bound<'py, PyAny> =
        compile_pipeline(request, invocation, (&planning_graph, &planning_names))?;
    let (python_graph, check_names) = python_selection(request, &discovered)?;
    let mut check_functions: Vec<Bound<'py, PyAny>> = Vec::new();
    for check in native_field(&discovered, "check_functions")?.try_iter()? {
        let check: Bound<'py, PyAny> = check?;
        if check_names.contains(native_field(&check, "name")?)? {
            check_functions.push(check);
        }
    }
    let selection: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::PythonSqlRunSelection,
        &[
            ("sql_keys", PyFrozenSet::empty(py)?.into_any()),
            ("python_node_names", PyFrozenSet::empty(py)?.into_any()),
        ],
    )?;
    let lifecycle_plan: Bound<'py, PyAny> =
        build_python_sql_run_lifecycle(&selection, &python_graph)?;
    let relation_refs: Bound<'py, PyAny> = call_with(
        &native_field(&python_graph, "selected_sql_refs")?,
        &[("selected_names", check_names.clone().into_any())],
    )?;
    let project: Bound<'py, PyAny> = native_field(&pipeline, "project")?;
    let plan_output: Bound<'py, PyAny> = native_field(&pipeline, "plan_output")?;
    let relation_targets: Bound<'py, PyAny> = object(
        py,
        build_python_relation_targets(
            py,
            Some(keywords(
                py,
                &[
                    ("adapter", adapter.clone()),
                    ("project", project.clone()),
                    ("plan_output", plan_output.clone()),
                    ("required_refs", relation_refs.clone()),
                ],
            )?),
        )?,
    )?;
    Ok(Prepared {
        check_functions: PyTuple::new(py, check_functions)?.into_any(),
        lifecycle_plan,
        relation_targets,
        relation_refs,
        defaults: (
            target_default(
                &project,
                &adapter,
                ("effective_target_database", "default_database"),
            )?,
            target_default(
                &project,
                &adapter,
                ("effective_target_schema", "default_schema"),
            )?,
        ),
        project_relations: object(
            py,
            build_project_relation_targets(
                py,
                Some(keywords(
                    py,
                    &[("adapter", adapter), ("plan_output", plan_output)],
                )?),
            )?,
        )?,
        python_graph,
        pipeline,
    })
}

/// The executor runtime of check dependencies and checks; ingress runs without project relations.
fn node_runtime<'py>(
    invocation: &Bound<'py, PyAny>,
    prepared: &Prepared<'py>,
    (connection, providers, with_project_relations): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, bool),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = invocation.py();
    let project: Bound<'py, PyAny> = native_field(&prepared.pipeline, "project")?;
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("adapter", native_field(invocation, "adapter")?),
        (
            "connection_config",
            native_field(invocation, "connection_config")?,
        ),
        ("connection", connection.clone()),
        ("run_id", native_field(&project, "run_id")?),
        ("target", native_field(&project, "effective_target_name")?),
        ("vars", native_field(&project, "effective_vars")?),
        ("is_reload", PyBool::new(py, false).to_owned().into_any()),
        ("default_database", prepared.defaults.0.clone()),
        ("default_schema", prepared.defaults.1.clone()),
        ("relation_targets", prepared.relation_targets.clone()),
    ];
    if with_project_relations {
        fields.push(("project_relations", prepared.project_relations.clone()));
    }
    fields.push(("providers", providers.clone()));
    call_with(&host::entry(py, ExecutorHost::PythonNodeRuntime)?, &fields)
}

fn record_run_state<'py>(
    invocation: &Bound<'py, PyAny>,
    (run_state, python_results, load_results): (
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
        Option<Bound<'py, PyAny>>,
    ),
    source_map: &Bound<'py, PyAny>,
) -> PyResult<()> {
    record_python_run_state_results(
        &native_field(invocation, "discovered_inputs")?,
        &run_state,
        (&python_results, load_results.as_ref()),
        Some(source_map),
    )
}

/// Run ingress loaders and read-side dependencies, then the checks themselves.
fn run_checks<'py>(
    invocation: &Bound<'py, PyAny>,
    prepared: &Prepared<'py>,
    (connection, providers, event_writer): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = invocation.py();
    let plan_output: Bound<'py, PyAny> = native_field(&prepared.pipeline, "plan_output")?;
    let source_map: Bound<'py, PyAny> = native_field(&plan_output, "source_map")?;
    let callbacks: Bound<'py, PyAny> = call_with(
        &host::entry(py, ExecutorHost::IngressCallbacks)?,
        &[("use_color", native_field(invocation, "use_color")?)],
    )?;
    let ingress: Bound<'py, PyAny> = call_with(
        &host::entry(py, ExecutorHost::ExecuteIngressPythonLoaderNodes)?,
        &[
            ("python_graph", prepared.python_graph.clone()),
            (
                "selected_python_names",
                native_field(&prepared.lifecycle_plan, "ingress_python_node_names")?,
            ),
            (
                "loader_functions",
                native_field(invocation, "discovered_inputs")?.getattr("loader_functions")?,
            ),
            ("source_map", source_map.clone()),
            (
                "runtime",
                node_runtime(invocation, prepared, (connection, providers, false))?,
            ),
            ("callbacks", callbacks),
        ],
    )?;
    let run_state: Bound<'py, PyAny> = native_field(&ingress, "run_state")?;
    let load_results: Bound<'py, PyAny> = native_field(&ingress, "load_results")?;
    let ingress_python: Bound<'py, PyAny> = native_field(&ingress, "python_results")?;
    record_run_state(
        invocation,
        (
            run_state.clone(),
            ingress_python.clone(),
            Some(load_results.clone()),
        ),
        &source_map,
    )?;
    let read_side: Bound<'py, PyAny> = run_read_side_dependencies(&ReadSideInputs {
        adapter: native_field(invocation, "adapter")?,
        connection_config: native_field(invocation, "connection_config")?,
        connection: connection.clone(),
        pipeline_result: prepared.pipeline.clone(),
        python_graph: prepared.python_graph.clone(),
        lifecycle_plan: prepared.lifecycle_plan.clone(),
        relation_targets: prepared.relation_targets.clone(),
        validation_refs: prepared.relation_refs.clone(),
        project_relations: prepared.project_relations.clone(),
        providers: providers.clone(),
    })?;
    record_run_state(
        invocation,
        (run_state.clone(), read_side.clone(), None),
        &source_map,
    )?;
    let upstream: Vec<Bound<'py, PyAny>> = ingress_python
        .try_iter()?
        .chain(read_side.try_iter()?)
        .collect::<PyResult<_>>()?;
    let by_loader: Bound<'py, PyAny> =
        load_results_by_loader_name(&source_map, &load_results)?.into_any();
    let on_complete: Bound<'py, PyAny> = Bound::new(
        py,
        CheckCompletionRelay {
            event_writer: event_writer.clone().unbind(),
            plan_output: plan_output.clone().unbind(),
        },
    )?
    .into_any();
    let check_callbacks: Bound<'py, PyAny> = call_with(
        &host::entry(py, ExecutorHost::PythonCheckCallbacks)?,
        &[("on_check_complete", on_complete)],
    )?;
    call_with(
        &host::entry(py, ExecutorHost::ExecutePythonChecks)?,
        &[
            ("check_functions", prepared.check_functions.clone()),
            ("python_graph", prepared.python_graph.clone()),
            (
                "upstream_python_results",
                PyTuple::new(py, upstream)?.into_any(),
            ),
            ("upstream_load_results", load_results),
            ("upstream_load_results_by_loader_name", by_loader),
            (
                "runtime",
                node_runtime(invocation, prepared, (connection, providers, true))?,
            ),
            ("run_state", run_state),
            (
                "require_upstream_results",
                PyBool::new(py, false).to_owned().into_any(),
            ),
            ("callbacks", check_callbacks),
        ],
    )
}

/// Record each finished check in the event output.
#[pyclass(module = "sqlbuild._native", frozen)]
struct CheckCompletionRelay {
    event_writer: Py<PyAny>,
    plan_output: Py<PyAny>,
}

#[pymethods]
impl CheckCompletionRelay {
    fn __call__(&self, py: Python<'_>, result: &Bound<'_, PyAny>) -> PyResult<()> {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("result", result)?;
        options.set_item("plan", self.plan_output.bind(py))?;
        options.set_item("command", "check")?;
        self.event_writer
            .bind(py)
            .call_method("write_build_result", (), Some(&options))?;
        Ok(())
    }
}

/// Connect, configure progress ordinals, run the checks, and close everything.
fn execute<'py>(
    invocation: &Bound<'py, PyAny>,
    prepared: &Prepared<'py>,
    providers: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = invocation.py();
    let adapter: Bound<'py, PyAny> = native_field(invocation, "adapter")?;
    let connection: Bound<'py, PyAny> = adapter
        .call_method1("connect", (native_field(invocation, "connection_config")?,))
        .map_err(|error| connection_failure(&adapter, &error))?;
    let event_writer: Bound<'py, PyAny> =
        Bound::new(py, ExecutionEventWriter::new(py, None)?)?.into_any();
    let executed: PyResult<Vec<Bound<'py, PyAny>>> = (|| {
        let projector: Bound<'py, PyAny> = current_native_progress_projector(py)?;
        if !projector.is_none() {
            let ordinals: Bound<'py, PyDict> = PyDict::new(py);
            let mut total: usize = 0;
            for check in prepared.check_functions.try_iter()? {
                total += 1;
                ordinals.set_item(native_field(&check?, "name")?, total)?;
            }
            let options: Bound<'py, PyDict> = PyDict::new(py);
            options.set_item("ordinals", ordinals)?;
            options.set_item("total", total)?;
            projector.call_method("configure_resources", (), Some(&options))?;
        }
        run_checks(
            invocation,
            prepared,
            (&connection, providers, &event_writer),
        )?
        .try_iter()?
        .collect()
    })();
    let writer_closed: PyResult<Bound<'py, PyAny>> = event_writer.call_method0("close");
    let connection_closed: PyResult<Bound<'py, PyAny>> =
        adapter.call_method1("close", (&connection,));
    let results: Vec<Bound<'py, PyAny>> = executed?;
    writer_closed?;
    connection_closed?;
    Ok(results)
}

/// Write rows, the summary, the runtime artifact and JSON output; returns the exit code.
fn complete<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Bound<'py, PyAny>,
    (prepared, results): (&Prepared<'py>, &[Bound<'py, PyAny>]),
) -> PyResult<i32> {
    let py: Python<'py> = request.py();
    let stream: Bound<'py, PyAny> = native_field(invocation, "progress_stream")?;
    let style: CliStyle = CliStyle {
        use_color: native_field(invocation, "use_color")?.extract()?,
    };
    write_results(
        &stream,
        results,
        (&prepared.check_functions, &prepared.python_graph),
        style,
    )?;
    let texts: Vec<CheckResultText> = results
        .iter()
        .map(check_result_text)
        .collect::<PyResult<_>>()?;
    stream.call_method1("write", (render_check_summary(&texts, style),))?;
    stream.call_method0("flush")?;
    let tuple: Bound<'py, PyAny> = PyTuple::new(py, results)?.into_any();
    write_python_check_runtime_target(
        &native_field(invocation, "effective_project_dir")?
            .call_method1("__truediv__", ("target",))?,
        &tuple,
    )?;
    object(
        py,
        write_execution_json_output(
            py,
            PyString::new(py, &results_json(results)?).into_any(),
            native_field(request, "json_output")?.is_truthy()?,
            native_field(request, "json_output_path")?,
        )?,
    )?;
    Ok(check_exit_code(&texts))
}

/// Run one `sqb check` request.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_check_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let invocation: Bound<'_, PyAny> = resolve_reported_command_invocation(
        request,
        &record_class(py, RecordId::CheckInvocation)?,
        (true, None),
    )?;
    let stream: Bound<'_, PyAny> = native_field(&invocation, "progress_stream")?;
    stream.call_method1("write", ("\n",))?;
    let header: Bound<'_, PyDict> = PyDict::new(py);
    header.set_item("stream", &stream)?;
    header.set_item("command", EXECUTION_COMMAND)?;
    header.set_item("target", py.None())?;
    header.set_item("concurrency", PyInt::new(py, 1))?;
    header.set_item("use_color", native_field(&invocation, "use_color")?)?;
    write_execution_header(py, Some(header))?;
    let prepared: Prepared<'_> = prepare(request, &invocation)?;
    let session: Bound<'_, PyAny> = call_with(
        &host::entry(py, UserCodeHost::BuildProviderSession)?,
        &[(
            "discovered_providers",
            native_field(&invocation, "discovered_inputs")?.getattr("providers")?,
        )],
    )?;
    let outcome: PyResult<i32> = (|| {
        let results: Vec<Bound<'_, PyAny>> = execute(
            &invocation,
            &prepared,
            &native_field(&session, "providers")?,
        )?;
        complete(request, &invocation, (&prepared, &results))
    })();
    let closed: PyResult<Bound<'_, PyAny>> = session.call_method0("close");
    let code: i32 = outcome?;
    closed?;
    Ok(code)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_check_command, module)?)?;
    Ok(())
}
