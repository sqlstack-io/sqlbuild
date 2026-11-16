//! Ingress Python before the build and read-side tasks and assets dispatched as SQL completes.

use std::collections::HashSet;
use std::sync::Mutex;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyclass, pymethods};
use sqlbuild_core::build_command::main::render_python_node_rows::render_python_node_rows;
use sqlbuild_core::build_command::models::PythonNodeRow;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::compile_driver::relation_targets::build_project_relation_targets;
use crate::bindings::_helpers::compile_driver::relation_targets::build_python_relation_targets;
use crate::bindings::_helpers::graph::python_lifecycle::build_python_sql_run_lifecycle;
use crate::bindings::_helpers::graph::python_node_graph::python_node_graph;
use crate::bindings::_helpers::planner_scope::loaders::build_intermediate_source_map;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::build_command::main::boolean::boolean;
use sqlbuild_python_base::build_command::main::emit::emit;
use sqlbuild_python_base::build_command::main::frozenset::frozenset;
use sqlbuild_python_base::build_command::main::or_none::or_none;
use sqlbuild_python_base::build_command::main::text::text;
use sqlbuild_python_base::build_command::main::tuple::tuple;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::RecordId;

const SUCCESS_STATUS: &str = "success";

/// The cursor window and output settings the Python lifecycle runs with.
pub(crate) struct LifecycleInputs<'py> {
    pub(crate) cursors: [Bound<'py, PyAny>; 4],
    pub(crate) on_node_start: Bound<'py, PyAny>,
    pub(crate) on_node_complete: Bound<'py, PyAny>,
    pub(crate) providers: Bound<'py, PyAny>,
}

fn node_rows(results: &Bound<'_, PyAny>) -> PyResult<Vec<PythonNodeRow>> {
    let mut rows: Vec<PythonNodeRow> = Vec::new();
    for result in results.try_iter()? {
        let result: Bound<'_, PyAny> = result?;
        rows.push(PythonNodeRow {
            kind: native_field(&result, "kind")?.getattr("value")?.extract()?,
            node_name: native_field(&result, "node_name")?.extract()?,
            status: native_field(&result, "status")?.str()?.extract()?,
            error_message: native_field(&result, "error_message")?.extract()?,
            skip_reason: native_field(&result, "skip_reason")?.extract()?,
        });
    }
    Ok(rows)
}

/// Write human-readable task and asset rows.
pub(crate) fn write_python_node_results(
    stream: &Bound<'_, PyAny>,
    results: &Bound<'_, PyAny>,
    use_color: bool,
) -> PyResult<()> {
    let rendered: String = render_python_node_rows(&node_rows(results)?, CliStyle { use_color });
    emit(stream, &rendered)
}

fn result_names(results: &Bound<'_, PyAny>) -> PyResult<HashSet<String>> {
    results
        .try_iter()?
        .map(|result| native_field(&result?, "node_name")?.extract())
        .collect()
}

fn names_set<'py>(py: Python<'py>, names: &[String]) -> PyResult<Bound<'py, PyAny>> {
    let values: Vec<Bound<'py, PyAny>> = names.iter().map(|name| text(py, name)).collect();
    frozenset(py, &values)
}

/// Mutable state of direct-mode Python lifecycle execution.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct DirectPythonLifecycle {
    plan: Py<PyAny>,
    discovered: Py<PyAny>,
    adapter: Py<PyAny>,
    stream: Py<PyAny>,
    use_color: bool,
    base_on_node_complete: Py<PyAny>,
    read_side_connection: Mutex<Option<Py<PyAny>>>,
    read_side_tracker: Option<Py<PyAny>>,
    ingress_python_results: Py<PyAny>,
    ingress_load_results: Py<PyAny>,
    ingress_loader_names: Py<PyAny>,
}

#[pymethods]
impl DirectPythonLifecycle {
    /// Record a finished SQL node, then dispatch and report newly ready Python nodes.
    fn on_node_complete(&self, py: Python<'_>, node_result: Bound<'_, PyAny>) -> PyResult<()> {
        let base: &Bound<'_, PyAny> = self.base_on_node_complete.bind(py);
        base.call1((&node_result,))?;
        let Some(tracker) = &self.read_side_tracker else {
            return Ok(());
        };
        let tracker: &Bound<'_, PyAny> = tracker.bind(py);
        let previous: HashSet<String> = result_names(&native_field(tracker, "results")?)?;
        tracker.call_method1("record_sql_result", (node_result,))?;
        let mut new_results: Vec<Bound<'_, PyAny>> = Vec::new();
        for result in native_field(tracker, "results")?.try_iter()? {
            let result: Bound<'_, PyAny> = result?;
            if !previous.contains(&native_field(&result, "node_name")?.extract::<String>()?) {
                new_results.push(result);
            }
        }
        self.report(py, &new_results)
    }
}

impl DirectPythonLifecycle {
    fn report(&self, py: Python<'_>, results: &[Bound<'_, PyAny>]) -> PyResult<()> {
        if results.is_empty() {
            return Ok(());
        }
        for result in results {
            self.base_on_node_complete.bind(py).call1((result,))?;
        }
        write_python_node_results(self.stream.bind(py), &tuple(py, results)?, self.use_color)
    }

    /// Ingress results, then read-side results.
    pub(crate) fn python_results<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut results: Vec<Bound<'py, PyAny>> = self
            .ingress_python_results
            .bind(py)
            .try_iter()?
            .collect::<PyResult<_>>()?;
        if let Some(tracker) = &self.read_side_tracker {
            for result in native_field(tracker.bind(py), "results")?.try_iter()? {
                results.push(result?);
            }
        }
        tuple(py, &results)
    }

    /// SQL-stage loaders, keeping dependencies Python ingress already satisfied only when needed.
    pub(crate) fn loader_functions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let loaders: Vec<Bound<'py, PyAny>> =
            native_field(self.discovered.bind(py), "loader_functions")?
                .try_iter()?
                .collect::<PyResult<_>>()?;
        let functions: Vec<Bound<'py, PyAny>> = loaders
            .iter()
            .map(|loader| native_field(loader, "function"))
            .collect::<PyResult<_>>()?;
        let functions: Bound<'py, PyAny> = frozenset(py, &functions)?;
        let ingress: &Bound<'py, PyAny> = self.ingress_loader_names.bind(py);
        let mut handoff: Vec<Bound<'py, PyAny>> = Vec::new();
        for loader in loaders {
            let keeps_all: bool = !ingress.contains(native_field(&loader, "name")?)?;
            let mut dependencies: Vec<Bound<'py, PyAny>> = Vec::new();
            for dependency in native_field(&loader, "depends_on")?.try_iter()? {
                let dependency: Bound<'py, PyAny> = dependency?;
                if keeps_all || functions.contains(&dependency)? {
                    dependencies.push(dependency);
                }
            }
            let changes: Bound<'py, PyDict> = PyDict::new(py);
            changes.set_item("depends_on", tuple(py, &dependencies)?)?;
            handoff.push(replaced_keywords(&loader, &changes)?);
        }
        tuple(py, &handoff)
    }

    fn ingress_keys<'py>(&self, py: Python<'py>, failed_only: bool) -> PyResult<Bound<'py, PyAny>> {
        let entries: Vec<Bound<'py, PyAny>> =
            native_field(self.plan.bind(py), "source_load_entries")?
                .try_iter()?
                .collect::<PyResult<_>>()?;
        let mut keys: Vec<Bound<'py, PyAny>> = Vec::new();
        for result in self.ingress_load_results.bind(py).try_iter()? {
            let result: Bound<'py, PyAny> = result?;
            if failed_only
                && native_field(&result, "status")?
                    .str()?
                    .extract::<String>()?
                    == SUCCESS_STATUS
            {
                continue;
            }
            let name: Bound<'py, PyAny> = native_field(&result, "source_name")?;
            for entry in &entries {
                if native_field(entry, "name")?.eq(&name)? {
                    keys.push(native_field(entry, "key")?);
                    break;
                }
            }
        }
        frozenset(py, &keys)
    }

    /// Plan keys of SQL source loads Python ingress already ran.
    pub(crate) fn precompleted_keys<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.ingress_keys(py, false)
    }

    /// Plan keys of SQL source loads Python ingress ran without success.
    pub(crate) fn blocked_keys<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.ingress_keys(py, true)
    }

    pub(crate) fn ingress_load_results<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.ingress_load_results.bind(py).clone()
    }

    /// Close the read-side connection and report Python nodes that never ran.
    pub(crate) fn finalize(&self, py: Python<'_>) -> PyResult<()> {
        let connection: Option<Py<PyAny>> = self
            .read_side_connection
            .lock()
            .map_err(|_| PyRuntimeError::new_err("read-side connection lock poisoned"))?
            .take();
        if let Some(connection) = connection {
            self.adapter.bind(py).call_method1("close", (connection,))?;
        }
        let Some(tracker) = &self.read_side_tracker else {
            return Ok(());
        };
        let finalized: Vec<Bound<'_, PyAny>> = tracker
            .bind(py)
            .call_method0("finalize_unrun_python_nodes")?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        self.report(py, &finalized)
    }
}

/// The models, seeds and source loads this run executes and reports results for.
fn scheduled_sql_names(plan: &Bound<'_, PyAny>) -> PyResult<HashSet<String>> {
    let scheduled: Bound<'_, PyAny> = imported(plan.py(), "builtins", "frozenset")?
        .call1((native_field(plan, "execution_order")?,))?;
    let mut names: HashSet<String> = HashSet::new();
    for kind in ["model_entries", "seed_entries", "source_load_entries"] {
        for entry in native_field(plan, kind)?.try_iter()? {
            let entry: Bound<'_, PyAny> = entry?;
            if scheduled.contains(native_field(&entry, "key")?)? {
                names.insert(native_field(&entry, "name")?.extract()?);
            }
        }
    }
    Ok(names)
}

struct RuntimeParts<'a, 'py> {
    adapter: &'a Bound<'py, PyAny>,
    connection_config: &'a Bound<'py, PyAny>,
    project: &'a Bound<'py, PyAny>,
    reload: bool,
    defaults: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    relations: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    inputs: &'a LifecycleInputs<'py>,
}

fn node_runtime<'py>(
    parts: &RuntimeParts<'_, 'py>,
    connection: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = connection.py();
    let [start_ts, end_ts, start_int, end_int] = &parts.inputs.cursors;
    call_with(
        &host_entry(py, ExecutorHost::PythonNodeRuntime)?,
        &[
            ("adapter", parts.adapter.clone()),
            ("connection_config", parts.connection_config.clone()),
            ("connection", connection.clone()),
            ("run_id", native_field(parts.project, "run_id")?),
            (
                "target",
                native_field(parts.project, "effective_target_name")?,
            ),
            ("vars", native_field(parts.project, "effective_vars")?),
            ("is_reload", boolean(py, parts.reload)),
            ("default_database", parts.defaults.0.clone()),
            ("default_schema", parts.defaults.1.clone()),
            ("start_cursor_ts", start_ts.clone()),
            ("end_cursor_ts", end_ts.clone()),
            ("start_cursor_int", start_int.clone()),
            ("end_cursor_int", end_int.clone()),
            ("relation_targets", parts.relations.0.clone()),
            ("project_relations", parts.relations.1.clone()),
            ("providers", parts.inputs.providers.clone()),
        ],
    )
}

/// Lifecycle plan, graph and source map shared by ingress and read-side execution.
struct LifecyclePlan<'py> {
    graph: Bound<'py, PyAny>,
    plan: Bound<'py, PyAny>,
    source_map: Bound<'py, PyAny>,
}

fn lifecycle_plan<'py>(
    discovered: &Bound<'py, PyAny>,
    pipeline: &Bound<'py, PyAny>,
) -> PyResult<LifecyclePlan<'py>> {
    let py: Python<'py> = discovered.py();
    let plan_output: Bound<'py, PyAny> = native_field(pipeline, "plan_output")?;
    let graph: Bound<'py, PyAny> = python_node_graph(discovered)?;
    let selection: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::PythonSqlRunSelection,
        &[
            (
                "sql_keys",
                imported(py, "builtins", "frozenset")?
                    .call1((native_field(&plan_output, "upstream_deps")?,))?,
            ),
            (
                "python_node_names",
                native_field(pipeline, "python_node_names")?,
            ),
        ],
    )?;
    let plan: Bound<'py, PyAny> = build_python_sql_run_lifecycle(&selection, &graph.clone())?;
    let source_type: Bound<'py, PyAny> = enum_member(py, RecordId::CompiledResourceType, "SOURCE")?;
    let mut keys: Vec<Bound<'py, PyAny>> = Vec::new();
    for name in native_field(&plan, "ingress_loader_names")?.try_iter()? {
        keys.push(new_record_with(
            py,
            RecordId::CompiledObjectKey,
            &[("name", name?), ("resource_type", source_type.clone())],
        )?);
    }
    let source_map: Bound<'py, PyAny> =
        imported(py, "builtins", "dict")?.call1((native_field(&plan_output, "source_map")?,))?;
    source_map.call_method1(
        "update",
        (build_intermediate_source_map(
            &native_field(pipeline, "project")?,
            &frozenset(py, &keys)?,
        )
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?,),
    )?;
    Ok(LifecyclePlan {
        graph,
        plan,
        source_map,
    })
}

fn task_asset_names(
    discovered: &Bound<'_, PyAny>,
    pipeline: &Bound<'_, PyAny>,
) -> PyResult<Vec<String>> {
    let mut known: HashSet<String> = HashSet::new();
    for kind in ["task_functions", "asset_functions"] {
        for node in native_field(discovered, kind)?.try_iter()? {
            known.insert(native_field(&node?, "name")?.extract()?);
        }
    }
    let mut names: Vec<String> = Vec::new();
    for name in native_field(pipeline, "python_node_names")?.try_iter()? {
        let name: String = name?.extract()?;
        if known.contains(&name) {
            names.push(name);
        }
    }
    Ok(names)
}

/// Execute ingress Python and prepare read-side dispatch for a direct build.
pub(crate) fn prepare_direct_python_lifecycle<'py>(
    context: &crate::bindings::_helpers::build_command::invocation::BuildContext<'py>,
    pipeline: &Bound<'py, PyAny>,
    inputs: &LifecycleInputs<'py>,
) -> PyResult<Bound<'py, DirectPythonLifecycle>> {
    let py: Python<'py> = context.py();
    let discovered: Bound<'py, PyAny> = context.get("discovered_inputs")?;
    let adapter: Bound<'py, PyAny> = context.get("adapter")?;
    let connection_config: Bound<'py, PyAny> = context.get("connection_config")?;
    let project: Bound<'py, PyAny> = native_field(pipeline, "project")?;
    let plan_output: Bound<'py, PyAny> = native_field(pipeline, "plan_output")?;
    let selected: Vec<String> = if context.request_flag("include_python")? {
        task_asset_names(&discovered, pipeline)?
    } else {
        Vec::new()
    };
    let lifecycle: LifecyclePlan<'py> = lifecycle_plan(&discovered, pipeline)?;
    let ingress_names: Bound<'py, PyAny> =
        native_field(&lifecycle.plan, "ingress_python_node_names")?;
    let required: Bound<'py, PyAny> = call_with(
        &native_field(&lifecycle.graph, "selected_sql_refs")?,
        &[(
            "selected_names",
            ingress_names.call_method1(
                "__or__",
                (native_field(
                    &lifecycle.plan,
                    "read_side_python_node_names",
                )?,),
            )?,
        )],
    )?;
    let relation_targets: Bound<'py, PyAny> = build_python_relation_targets(
        py,
        Some(keywords(
            py,
            &[
                ("adapter", adapter.clone()),
                ("project", project.clone()),
                ("plan_output", plan_output.clone()),
                ("required_refs", required),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let project_relations: Bound<'py, PyAny> = build_project_relation_targets(
        py,
        Some(keywords(
            py,
            &[
                ("adapter", adapter.clone()),
                ("plan_output", plan_output.clone()),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let parts: RuntimeParts<'_, 'py> = RuntimeParts {
        adapter: &adapter,
        connection_config: &connection_config,
        project: &project,
        reload: context.request_flag("reload_sources")?,
        defaults: (
            or_none(native_field(&project, "effective_target_database")?, || {
                adapter.call_method0("default_database")
            })?,
            or_none(native_field(&project, "effective_target_schema")?, || {
                adapter.call_method0("default_schema")
            })?,
        ),
        relations: (relation_targets, project_relations),
        inputs,
    };
    let mut ingress_python: Bound<'py, PyAny> = tuple(py, &[])?;
    let mut ingress_loads: Bound<'py, PyAny> = tuple(py, &[])?;
    if ingress_names.is_truthy()? {
        let connection: Bound<'py, PyAny> =
            adapter.call_method1("connect", (connection_config.clone(),))?;
        let executed: PyResult<Bound<'py, PyAny>> = (|| {
            let callbacks: Bound<'py, PyAny> = call_with(
                &host_entry(py, ExecutorHost::IngressCallbacks)?,
                &[
                    ("use_color", boolean(py, context.use_color)),
                    ("on_node_start", inputs.on_node_start.clone()),
                    ("on_node_complete", inputs.on_node_complete.clone()),
                ],
            )?;
            call_with(
                &host_entry(py, ExecutorHost::RunIngressPythonLoaderNodes)?,
                &[
                    ("python_graph", lifecycle.graph.clone()),
                    ("selected_python_names", ingress_names.clone()),
                    (
                        "loader_functions",
                        native_field(&discovered, "loader_functions")?,
                    ),
                    ("source_map", lifecycle.source_map.clone()),
                    ("runtime", node_runtime(&parts, &connection)?),
                    ("callbacks", callbacks),
                ],
            )
        })();
        let closed: PyResult<Bound<'py, PyAny>> = adapter.call_method1("close", (connection,));
        let executed: Bound<'py, PyAny> = executed?;
        closed?;
        ingress_python = native_field(&executed, "python_results")?;
        ingress_loads = native_field(&executed, "load_results")?;
        write_python_node_results(&context.stream, &ingress_python, context.use_color)?;
    }
    let ingress_set: HashSet<String> = ingress_names
        .try_iter()?
        .map(|name| name?.extract())
        .collect::<PyResult<_>>()?;
    let ran: HashSet<String> = result_names(&ingress_python)?;
    let read_side: Vec<String> = selected
        .into_iter()
        .filter(|name| !ingress_set.contains(name) && !ran.contains(name))
        .collect();
    let mut read_side_connection: Option<Py<PyAny>> = None;
    let mut read_side_tracker: Option<Py<PyAny>> = None;
    if !read_side.is_empty() {
        let connection: Bound<'py, PyAny> =
            adapter.call_method1("connect", (connection_config.clone(),))?;
        read_side_connection = Some(connection.clone().unbind());
        let mut scheduled: HashSet<String> = scheduled_sql_names(&plan_output)?;
        for result in ingress_loads.try_iter()? {
            scheduled.insert(native_field(&result?, "source_name")?.extract()?);
        }
        let scheduled: Vec<String> = scheduled.into_iter().collect();
        let tracker: Bound<'py, PyAny> = call_with(
            &host_entry(py, ExecutorHost::CreateReadSidePythonExecutionTracker)?,
            &[
                ("python_graph", lifecycle.graph.clone()),
                ("selected_python_names", names_set(py, &read_side)?),
                ("scheduled_sql_names", names_set(py, &scheduled)?),
                ("runtime", node_runtime(&parts, &connection)?),
            ],
        )?;
        for result in ingress_loads.try_iter()? {
            tracker.call_method1("record_sql_result", (result?,))?;
        }
        tracker.call_method0("dispatch_ready_python_nodes")?;
        let initial: Bound<'py, PyAny> = native_field(&tracker, "results")?;
        if initial.is_truthy()? {
            write_python_node_results(&context.stream, &initial, context.use_color)?;
        }
        read_side_tracker = Some(tracker.unbind());
    }
    Bound::new(
        py,
        DirectPythonLifecycle {
            plan: plan_output.unbind(),
            discovered: discovered.unbind(),
            adapter: adapter.clone().unbind(),
            stream: context.stream.clone().unbind(),
            use_color: context.use_color,
            base_on_node_complete: inputs.on_node_complete.clone().unbind(),
            read_side_connection: Mutex::new(read_side_connection),
            read_side_tracker,
            ingress_python_results: ingress_python.unbind(),
            ingress_load_results: ingress_loads.unbind(),
            ingress_loader_names: native_field(&lifecycle.plan, "ingress_loader_names")?.unbind(),
        },
    )
}
