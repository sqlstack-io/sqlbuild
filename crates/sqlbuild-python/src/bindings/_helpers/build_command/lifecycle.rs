//! Ingress Python before the build and read-side assets dispatched as SQL completes.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyclass, pymethods};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::python_plan::models::RoundTripLoader;
use sqlbuild_core::build_command::main::render_python_node_rows::render_python_node_rows;
use sqlbuild_core::build_command::models::PythonNodeRow;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_host::entries::models::ExecutorHost;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::build_command::main::boolean::boolean;
use sqlbuild_python_base::build_command::main::emit::emit;
use sqlbuild_python_base::build_command::main::frozenset::frozenset;
use sqlbuild_python_base::build_command::main::text::text;
use sqlbuild_python_base::build_command::main::tuple::tuple;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_compile::compile_driver::main::setup::build_project_relation_targets::build_project_relation_targets;
use sqlbuild_python_compile::compile_driver::main::setup::build_python_relation_targets::build_python_relation_targets;
use sqlbuild_python_compile::compile_driver::main::setup::project_target_settings::project_target_settings;
use sqlbuild_python_compile::compile_driver::models::TargetSettingsValues;
use sqlbuild_python_project::graph::main::python_nodes::build_python_sql_run_lifecycle::build_python_sql_run_lifecycle;
use sqlbuild_python_project::graph::main::python_nodes::native_run_lifecycle::native_run_lifecycle;
use sqlbuild_python_project::graph::main::python_nodes::python_node_graph::python_node_graph;
use sqlbuild_python_project::planner_scope::main::build_intermediate_source_map::build_intermediate_source_map;

const ASSET_KIND: &str = "asset";
const SUCCESS_STATUS: &str = "success";
const SKIPPED_STATUS: &str = "skipped";
const HARD_SKIP: &str = "hard";
const SOFT_SKIP: &str = "soft";

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

/// Write human-readable asset rows.
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

/// Reports one read-side Python node to progress, and so to a worker's server, and writes its row.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct PythonNodeReporter {
    on_node_complete: Py<PyAny>,
    stream: Py<PyAny>,
    use_color: bool,
    worker: Option<Arc<WorkerBuild>>,
}

#[pymethods]
impl PythonNodeReporter {
    /// A worker records an item that was running when the run was cancelled as cancelled.
    fn cancelled(&self, py: Python<'_>, name: &str) -> PyResult<()> {
        match &self.worker {
            Some(worker) => {
                worker.finish_key(py, &format!("{ASSET_KIND}:{name}"), NodeOutcome::Cancelled)
            }
            None => Ok(()),
        }
    }

    fn __call__(&self, py: Python<'_>, result: Bound<'_, PyAny>) -> PyResult<()> {
        self.on_node_complete.bind(py).call1((&result,))?;
        write_python_node_results(self.stream.bind(py), &tuple(py, &[result])?, self.use_color)
    }
}

/// Mutable state of direct-mode Python lifecycle execution.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct DirectPythonLifecycle {
    plan: Py<PyAny>,
    discovered: Py<PyAny>,
    adapter: Py<PyAny>,
    reporter: Py<PythonNodeReporter>,
    read_side_connection: Mutex<Option<Py<PyAny>>>,
    read_side_tracker: Option<Py<PyAny>>,
    ingress_python_results: Py<PyAny>,
    ingress_load_results: Py<PyAny>,
    ingress_loader_names: Py<PyAny>,
    /// Each round-trip loader's selected upstream assets, `(kind, name)`, and whether each is on its frontier.
    round_trip_gates: RoundTripGates,
    /// Each round-trip loader's source load key and the SQL keys it waits for.
    round_trip_waits: Vec<(ObjectKey, Vec<ObjectKey>)>,
    /// Whether the build runs read-side assets; `--no-python` turns this off.
    include_python: bool,
}

#[pymethods]
impl DirectPythonLifecycle {
    /// Record a finished SQL node, then dispatch newly ready Python nodes, each reported as it finishes.
    fn on_node_complete(&self, py: Python<'_>, node_result: Bound<'_, PyAny>) -> PyResult<()> {
        self.reporter
            .get()
            .on_node_complete
            .bind(py)
            .call1((&node_result,))?;
        let Some(tracker) = &self.read_side_tracker else {
            return Ok(());
        };
        tracker
            .bind(py)
            .call_method1("record_sql_result", (node_result,))?;
        Ok(())
    }
}

impl DirectPythonLifecycle {
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

    /// The gate a build consults before each source load, for round-trip loaders' upstreams.
    pub(crate) fn source_load_gate<'py>(
        slf: &Bound<'py, Self>,
    ) -> PyResult<Bound<'py, SourceLoadGate>> {
        Bound::new(
            slf.py(),
            SourceLoadGate {
                lifecycle: slf.clone().unbind(),
            },
        )
    }

    /// `plan` with each round-trip loader's source load waiting for the SQL its upstream reads.
    pub(crate) fn ordered_plan<'py>(
        &self,
        plan: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if self.round_trip_waits.is_empty() {
            return Ok(plan.clone());
        }
        let py: Python<'py> = plan.py();
        let upstream: Bound<'py, PyDict> = native_field(plan, "upstream_deps")?
            .cast_into::<PyDict>()?
            .copy()?;
        let downstream: Bound<'py, PyDict> = native_field(plan, "downstream_deps")?
            .cast_into::<PyDict>()?
            .copy()?;
        let mut planned: HashMap<ObjectKey, Bound<'py, PyAny>> = HashMap::new();
        for key in native_field(plan, "execution_order")?.try_iter()? {
            let key: Bound<'py, PyAny> = key?;
            planned.insert(
                (
                    native_field(&key, "resource_type")?.extract()?,
                    native_field(&key, "name")?.extract()?,
                ),
                key,
            );
        }
        for (source, waits) in &self.round_trip_waits {
            let Some(source) = planned.get(source) else {
                continue;
            };
            let waits: Vec<&Bound<'py, PyAny>> =
                waits.iter().filter_map(|wait| planned.get(wait)).collect();
            let mut deps: Vec<Bound<'py, PyAny>> = match upstream.get_item(source)? {
                Some(existing) => existing.try_iter()?.collect::<PyResult<_>>()?,
                None => Vec::new(),
            };
            for wait in waits {
                if !deps.iter().any(|dep| dep.eq(wait).unwrap_or(false)) {
                    deps.push(wait.clone());
                }
                let mut dependents: Vec<Bound<'py, PyAny>> = match downstream.get_item(wait)? {
                    Some(existing) => existing.try_iter()?.collect::<PyResult<_>>()?,
                    None => Vec::new(),
                };
                if !dependents.iter().any(|dep| dep.eq(source).unwrap_or(false)) {
                    dependents.push(source.clone());
                }
                downstream.set_item(wait, tuple(py, &dependents)?)?;
            }
            upstream.set_item(source, tuple(py, &deps)?)?;
        }
        let changes: Bound<'py, PyDict> = PyDict::new(py);
        changes.set_item("upstream_deps", upstream)?;
        changes.set_item("downstream_deps", downstream)?;
        replaced_keywords(plan, &changes)
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
        let finalized: Bound<'_, PyAny> = tracker
            .bind(py)
            .call_method0("finalize_unrun_python_nodes")?;
        for result in finalized.try_iter()? {
            self.reporter.get().__call__(py, result?)?;
        }
        Ok(())
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
    target_config: TargetSettingsValues,
    relations: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    inputs: &'a LifecycleInputs<'py>,
    /// Each Python node's job warehouse, by name.
    warehouses: BTreeMap<String, String>,
    /// The first server run of a worker's rerun chain; `None` outside a worker.
    root_run_id: Option<String>,
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
            ("target_config", parts.target_config.record(py)?),
            ("default_database", parts.defaults.0.clone()),
            ("default_schema", parts.defaults.1.clone()),
            ("start_cursor_ts", start_ts.clone()),
            ("end_cursor_ts", end_ts.clone()),
            ("start_cursor_int", start_int.clone()),
            ("end_cursor_int", end_int.clone()),
            ("relation_targets", parts.relations.0.clone()),
            ("project_relations", parts.relations.1.clone()),
            ("providers", parts.inputs.providers.clone()),
            (
                "node_warehouses",
                pyo3::IntoPyObject::into_pyobject(parts.warehouses.clone(), py)?.into_any(),
            ),
            (
                "chain_run_id",
                pyo3::IntoPyObject::into_pyobject(parts.root_run_id.clone(), py)?.into_any(),
            ),
        ],
    )
}

/// Lifecycle plan, graph and source map shared by ingress and read-side execution.
struct LifecyclePlan<'py> {
    graph: Bound<'py, PyAny>,
    plan: Bound<'py, PyAny>,
    source_map: Bound<'py, PyAny>,
    round_trips: Vec<RoundTripLoader>,
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
    let round_trips: Vec<RoundTripLoader> =
        native_run_lifecycle(&selection, &graph)?.round_trip_loaders;
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
        round_trips,
    })
}

fn asset_names(
    discovered: &Bound<'_, PyAny>,
    pipeline: &Bound<'_, PyAny>,
) -> PyResult<Vec<String>> {
    let mut known: HashSet<String> = HashSet::new();
    for node in native_field(discovered, "asset_functions")?.try_iter()? {
        known.insert(native_field(&node?, "name")?.extract()?);
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
    let include_python: bool = context.request_flag("include_python")?;
    let selected: Vec<String> = if include_python {
        asset_names(&discovered, pipeline)?
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
    let target_config: TargetSettingsValues =
        project_target_settings(&discovered, &project, (&connection_config, &adapter))?;
    let parts: RuntimeParts<'_, 'py> = RuntimeParts {
        adapter: &adapter,
        connection_config: &connection_config,
        project: &project,
        reload: context.request_flag("reload_sources")?,
        defaults: target_config.node_locations(&adapter)?,
        target_config,
        relations: (relation_targets, project_relations),
        inputs,
        warehouses: native_field(pipeline, "python_node_warehouses")?.extract()?,
        root_run_id: context
            .worker
            .as_ref()
            .map(|worker| worker.root_run_id().to_string()),
    };
    let reporter: Bound<'py, PythonNodeReporter> = Bound::new(
        py,
        PythonNodeReporter {
            on_node_complete: inputs.on_node_complete.clone().unbind(),
            stream: context.stream.clone().unbind(),
            use_color: context.use_color,
            worker: context.worker.clone(),
        },
    )?;
    let on_node_cancelled: Bound<'py, PyAny> = native_field(&reporter, "cancelled")?;
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
                    ("on_node_cancelled", on_node_cancelled.clone()),
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
                ("on_node_complete", reporter.clone().into_any()),
                ("on_node_cancelled", on_node_cancelled.clone()),
            ],
        )?;
        for result in ingress_loads.try_iter()? {
            tracker.call_method1("record_sql_result", (result?,))?;
        }
        tracker.call_method0("dispatch_ready_python_nodes")?;
        read_side_tracker = Some(tracker.unbind());
    }
    let (round_trip_gates, round_trip_waits) = round_trip_ordering(&lifecycle, &plan_output)?;
    let prepared = Bound::new(
        py,
        DirectPythonLifecycle {
            plan: plan_output.unbind(),
            discovered: discovered.unbind(),
            adapter: adapter.clone().unbind(),
            reporter: reporter.unbind(),
            read_side_connection: Mutex::new(read_side_connection),
            read_side_tracker,
            ingress_python_results: ingress_python.unbind(),
            ingress_load_results: ingress_loads.unbind(),
            ingress_loader_names: native_field(&lifecycle.plan, "ingress_loader_names")?.unbind(),
            round_trip_gates,
            round_trip_waits,
            include_python,
        },
    )?;
    Ok(prepared)
}

type RoundTripGates = HashMap<String, Vec<(ObjectKey, bool)>>;
type RoundTripWaits = Vec<(ObjectKey, Vec<ObjectKey>)>;

/// Each round-trip loader's gating upstreams, and its source load key with the SQL it waits for.
fn round_trip_ordering(
    lifecycle: &LifecyclePlan<'_>,
    plan_output: &Bound<'_, PyAny>,
) -> PyResult<(RoundTripGates, RoundTripWaits)> {
    let nodes: Bound<'_, PyAny> = native_field(&lifecycle.graph, "nodes_by_name")?;
    let mut gates: RoundTripGates = HashMap::new();
    let mut waits: RoundTripWaits = Vec::new();
    for loader in &lifecycle.round_trips {
        let mut kinds: Vec<(ObjectKey, bool)> = Vec::new();
        for name in &loader.gates {
            let kind: String = native_field(&nodes.get_item(name)?, "kind")?.extract()?;
            kinds.push(((kind, name.clone()), loader.frontier.contains(name)));
        }
        gates.insert(loader.name.clone(), kinds);
        for entry in native_field(plan_output, "source_load_entries")?.try_iter()? {
            let entry: Bound<'_, PyAny> = entry?;
            if native_field(&entry, "loader")?.extract::<String>()? == loader.name {
                let key: Bound<'_, PyAny> = native_field(&entry, "key")?;
                waits.push((
                    (
                        native_field(&key, "resource_type")?.extract()?,
                        native_field(&key, "name")?.extract()?,
                    ),
                    loader.waits_for.clone(),
                ));
            }
        }
    }
    Ok((gates, waits))
}

/// The build's source-load gate: why a round-trip loader's load is skipped, and how.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct SourceLoadGate {
    lifecycle: Py<DirectPythonLifecycle>,
}

#[pymethods]
impl SourceLoadGate {
    /// Why a round-trip load is skipped: `hard` for any failed or hard-skipped gate, else `soft` unless a frontier gate succeeded.
    fn __call__(
        &self,
        py: Python<'_>,
        source: Bound<'_, PyAny>,
    ) -> PyResult<Option<(String, &'static str)>> {
        let loader: Option<String> = native_field(&source, "loader")?.extract()?;
        let lifecycle: &DirectPythonLifecycle = self.lifecycle.get();
        let Some(gates) = loader.and_then(|loader| lifecycle.round_trip_gates.get(&loader)) else {
            return Ok(None);
        };
        let mut statuses: HashMap<String, (String, Option<String>)> = HashMap::new();
        for result in lifecycle.python_results(py)?.try_iter()? {
            let result: Bound<'_, PyAny> = result?;
            let skip_mode: Bound<'_, PyAny> = native_field(&result, "skip_mode")?;
            statuses.insert(
                native_field(&result, "node_name")?.extract()?,
                (
                    native_field(&result, "status")?.str()?.extract()?,
                    (!skip_mode.is_none())
                        .then(|| skip_mode.str()?.extract())
                        .transpose()?,
                ),
            );
        }
        let mut succeeded: bool = false;
        let mut soft_reason: Option<String> = None;
        let mut not_run_reason: Option<String> = None;
        for ((kind, name), frontier) in gates {
            let status = statuses
                .get(name)
                .map(|(status, mode)| (status.as_str(), mode.as_deref()));
            let outcome: &str = match status {
                Some((SUCCESS_STATUS, _)) => {
                    succeeded |= *frontier;
                    continue;
                }
                Some((SKIPPED_STATUS, Some(SOFT_SKIP))) => "was skipped",
                Some((SKIPPED_STATUS, _)) => {
                    return Ok(Some((
                        format!("Upstream {kind} '{name}' was skipped"),
                        HARD_SKIP,
                    )));
                }
                Some(_) => {
                    return Ok(Some((
                        format!("Upstream {kind} '{name}' failed"),
                        HARD_SKIP,
                    )));
                }
                None if !lifecycle.include_python => {
                    not_run_reason.get_or_insert_with(|| {
                        format!(
                            "Upstream {kind} '{name}' did not run; --no-python skips read-side assets"
                        )
                    });
                    continue;
                }
                None => "did not run; the SQL it reads was not rebuilt",
            };
            soft_reason.get_or_insert_with(|| format!("Upstream {kind} '{name}' {outcome}"));
        }
        if let Some(reason) = not_run_reason {
            return Ok(Some((reason, SOFT_SKIP)));
        }
        if succeeded {
            return Ok(None);
        }
        Ok(soft_reason.map(|reason| (reason, SOFT_SKIP)))
    }
}
