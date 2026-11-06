//! Clone planning: destination and origin entries, and deferred-clone graph traversal.

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::compiled_views::function_reads;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::seed_reads;
use crate::bindings::_helpers::compiled_views::source_reads;
use crate::bindings::_helpers::graph::keys::KeyCache;
use crate::bindings::_helpers::graph::node_set::PyNodeSet;
use crate::bindings::_helpers::graph::project_graph::NativeProjectGraph;
use crate::bindings::_helpers::graph::views::edge_dict;
use crate::bindings::_helpers::model_config::typed_config::{config_node_of, model_values};
use crate::bindings::_helpers::planner_entries::models::ModelPlanContext;
use crate::bindings::_helpers::planner_entries::values::{
    native, planner_model, planner_type, string,
};
use crate::bindings::_helpers::planner_identity::graph::{execution_deps, execution_order};
use crate::bindings::_helpers::planner_identity::metadata::build_compiled_function_fingerprint_sql;
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_resolution::locations::build_function_locations;
use crate::bindings::_helpers::planner_resolution::locations::build_model_locations;
use crate::bindings::_helpers::planner_resolution::locations::build_seed_locations;
use crate::bindings::_helpers::planner_resolution::models::ModelSqlRequest;
use crate::bindings::_helpers::planner_resolution::orchestration::resolve_function_sql;
use crate::bindings::_helpers::planner_resolution::orchestration::{Resolution, resolve_model_sql};
use crate::bindings::_helpers::planner_selection::selectors::resolve_graph_selection;
use crate::bindings::_helpers::planner_strategy::strategy::materialization_of;
use crate::bindings::_helpers::planner_warehouse::source_columns::gather_source_columns;
use crate::bindings::_helpers::records::construct::{enum_member, new_record};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::PyRef;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PySet, PySetMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::graph::main::downstream_edges::downstream_edges;
use sqlbuild_core::graph_walk::main::clone_ancestors::clone_ancestors;
use sqlbuild_core::graph_walk::models::ClonePolicy;
use sqlbuild_model_config::config_values::models::ConfigNode;

const ENABLED_KEY: &str = "enabled";

const VIEW_MATERIALIZATION: &str = "view";
const GRAPH_ARGUMENTS: [&str; 4] = ["selected", "upstream", "is_clonable", "is_view"];
/// The clone phase that reads source columns from the warehouse.
const SOURCE_INSPECTION_PHASE: &str = "source_inspection";
/// The clone phase that plans entries from the compiled projects.
const PLANNING_PHASE: &str = "planning";

/// Each clone phase name with its seconds, in the order the phases ran.
pub(crate) type PhaseTimings = Vec<(String, f64)>;

/// Whether a model is explicitly disabled with a boolean `enabled = false`.
fn disabled_model(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(matches!(
        *config_node_of(&model_values(model)?, ENABLED_KEY)?,
        ConfigNode::Bool(false)
    ))
}

fn tuple_of<'py>(py: Python<'py>, items: Vec<Bound<'py, PyAny>>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(py, items)?.into_any())
}

/// Entries keyed by node key, returned in `execution_order`.
fn in_order<'py>(
    entries: &Bound<'py, PyDict>,
    execution_order: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut ordered: Vec<Bound<'py, PyAny>> = Vec::new();
    for key in execution_order.try_iter()? {
        if let Some(entry) = entries.get_item(key?)? {
            ordered.push(entry);
        }
    }
    tuple_of(entries.py(), ordered)
}

fn clone_plan_output<'py>(
    project: &Bound<'py, PyAny>,
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let upstream = execution_deps(project)?;
    let order = execution_order(py, &upstream, &[])?;
    let cache = KeyCache::new(py)?;
    let downstream = edge_dict(&cache, &downstream_edges(&upstream))?.into_any();
    let upstream = edge_dict(&cache, &upstream)?.into_any();
    let selected = object(
        py,
        resolve_graph_selection(
            native_field(project, "lineage_graph")?.extract::<PyRef<NativeProjectGraph>>()?,
            select,
            exclude,
        )?,
    )?;
    planner_model(
        py,
        RecordId::PlanOutput,
        &[
            (
                "execution_order",
                PyTuple::new(py, cache.keys(&order)?)?.into_any(),
            ),
            ("selected_keys", selected),
            ("upstream_deps", upstream),
            ("downstream_deps", downstream),
        ],
    )
}

/// Locations and source columns that destination SQL resolves against.
struct CloneResolution<'py> {
    model_locations: Bound<'py, PyAny>,
    seed_locations: Bound<'py, PyAny>,
    function_locations: Bound<'py, PyAny>,
    source_map: Bound<'py, PyAny>,
    source_warehouse_columns: Bound<'py, PyAny>,
}

/// Whether a selected, enabled model is recreated from its SQL rather than copied.
fn recreated_view(model: &Bound<'_, PyAny>, selected: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(selected.contains(model_reads::model_key(model)?)?
        && !disabled_model(model)?
        && materialization_of(model)?.eq(VIEW_MATERIALIZATION)?)
}

/// The entries of the sources whose columns recreated view and function SQL resolves against.
fn resolved_source_entries<'py>(
    (project, plan): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    source_project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let selected = native_field(plan, "selected_keys")?;
    let source_type = enum_member(py, RecordId::CompiledResourceType, "SOURCE")?;
    let read = PySet::empty(py)?;
    let mut readers: Vec<Bound<'py, PyAny>> = Vec::new();
    for model in project_reads::models(project)?.try_iter()? {
        let model = model?;
        if recreated_view(&model, &selected)? {
            readers.push(model);
        }
    }
    for function in project_reads::functions(project)?.try_iter()? {
        let function = function?;
        if selected.contains(function_reads::key(&function)?)? {
            readers.push(function);
        }
    }
    for reader in readers {
        for dep in native_field(&reader, "deps")?.try_iter()? {
            let dep = dep?;
            if native_field(&dep, "resource_type")?.eq(&source_type)? {
                read.add(native_field(&dep, "name")?)?;
            }
        }
    }
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for source in native_field(source_project, "sources")?.try_iter()? {
        let source = source?;
        if read.contains(source_reads::name(&source)?)? {
            entries.push(source_reads::source_entry(&source)?);
        }
    }
    tuple_of(py, entries)
}

fn clone_resolution<'py>(
    (project, plan): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    source_project: &Bound<'py, PyAny>,
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<CloneResolution<'py>> {
    let py = project.py();
    let model_locations = object(py, build_model_locations(&project_reads::models(project)?)?)?;
    let seed_locations = object(py, build_seed_locations(&project_reads::seeds(project)?)?)?;
    let function_locations = object(
        py,
        build_function_locations(&project_reads::functions(project)?)?,
    )?;
    let source_map = PyDict::new(py);
    for source in native_field(source_project, "sources")?.try_iter()? {
        let source = source?;
        source_map.set_item(
            source_reads::name(&source)?,
            source_reads::source_entry(&source)?,
        )?;
    }
    let source_warehouse_columns = object(
        py,
        gather_source_columns(
            source_project,
            adapter,
            connection,
            Some(resolved_source_entries((project, plan), source_project)?),
        )?,
    )?;
    Ok(CloneResolution {
        model_locations,
        seed_locations,
        function_locations,
        source_map: source_map.into_any(),
        source_warehouse_columns,
    })
}

fn model_entry<'py>(
    model: &Bound<'py, PyAny>,
    materialization_type: Bound<'py, PyAny>,
    resolved_sql: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = model.py();
    let action = if materialization_type.eq(VIEW_MATERIALIZATION)? {
        "CREATE_VIEW"
    } else {
        "CREATE_TABLE"
    };
    planner_model(
        py,
        RecordId::ModelPlanEntry,
        &[
            ("key", model_reads::model_key(model)?),
            ("name", model_reads::name_object(model)?),
            ("relative_path", native_field(model, "relative_path")?),
            ("materialization_type", materialization_type),
            ("action", planner_type(py, RecordId::PlanAction, action)?),
            (
                "reason",
                planner_type(py, RecordId::PlanReason, "NO_CHANGE")?,
            ),
            ("destination", model_reads::destination(model)?),
            ("fingerprint_query_sql", string(py, "")),
            ("resolved_sql", resolved_sql),
            ("logical_ddl", string(py, "")),
        ],
    )
}

fn clone_model_entries<'py>(
    (project, plan): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    adapter: &Bound<'py, PyAny>,
    resolution: &CloneResolution<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let selected = native_field(plan, "selected_keys")?;
    let entries = PyDict::new(py);
    for model in project_reads::models(project)?.try_iter()? {
        let model = model?;
        if !selected.contains(model_reads::model_key(&model)?)? || disabled_model(&model)? {
            continue;
        }
        let materialization_type = materialization_of(&model)?;
        let resolved_sql = if materialization_type.eq(VIEW_MATERIALIZATION)? {
            let functions = PyDict::new(py);
            for function in project_reads::functions(project)?.try_iter()? {
                let function = function?;
                functions.set_item(function_reads::name(&function)?, function)?;
            }
            let context = Bound::new(
                py,
                ModelPlanContext {
                    model_locations: resolution.model_locations.clone().unbind(),
                    models_by_name: PyDict::new(py).into_any().into(),
                    functions_by_name: functions.into_any().unbind(),
                    seed_locations: resolution.seed_locations.clone().unbind(),
                    function_locations: resolution.function_locations.clone().unbind(),
                    source_map: resolution.source_map.clone().unbind(),
                    source_warehouse_columns: resolution.source_warehouse_columns.clone().unbind(),
                    star_exclude_keyword: adapter.call_method0("star_exclude_keyword")?.unbind(),
                    runtime_cursor_producer_names: PyFrozenSet::empty(py)?.into_any().unbind(),
                    future_cursor_config: py.None(),
                    start_cursor_config: py.None(),
                    invocation_time: py.None(),
                },
            )?
            .into_any();
            let sql = resolve_model_sql(
                &Resolution::of_context(adapter.clone(), &context, py.None().into_bound(py))?,
                &context,
                &ModelSqlRequest {
                    model: model.clone(),
                    snapshot: native(py, RecordId::WarehouseSnapshot, &[])?,
                    backfill: native(
                        py,
                        RecordId::BackfillResult,
                        &[(
                            "action",
                            planner_type(py, RecordId::BackfillAction, "FORWARD_ONLY")?,
                        )],
                    )?,
                    full_refresh: false,
                    cursor_overrides: (py.None().into_bound(py), py.None().into_bound(py)),
                    replaces_relation: false,
                    full_rebuild_cause: None,
                },
            )?;
            string(py, &sql)
        } else {
            string(py, "")
        };
        entries.set_item(
            model_reads::model_key(&model)?,
            model_entry(&model, materialization_type, resolved_sql)?,
        )?;
    }
    in_order(&entries, &native_field(plan, "execution_order")?)
}

fn clone_source_entries<'py>(
    project: &Bound<'py, PyAny>,
    (selected, execution_order): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    adapter: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let entries = PyDict::new(py);
    for source in project_reads::sources(project)?.try_iter()? {
        let source = source?;
        let entry = source_reads::source_entry(&source)?;
        if !selected.contains(source_reads::key(&source)?)?
            || !native_field(&entry, "managed")?.is_truthy()?
            || native_field(&entry, "loader")?.is_none()
            || !native_field(&entry, "expression")?.is_none()
        {
            continue;
        }
        let table = native_field(&entry, "table")?;
        let relation_name = if table.is_truthy()? {
            table
        } else {
            native_field(&entry, "name")?
        };
        let kwargs = PyDict::new(py);
        kwargs.set_item("database", native_field(&entry, "database")?)?;
        kwargs.set_item("schema", native_field(&entry, "schema")?)?;
        kwargs.set_item("name", &relation_name)?;
        let qualified = native_field(adapter, "render_qualified_name")?.call((), Some(&kwargs))?;
        let destination = new_record(
            py,
            RecordId::CompiledRelationLocation,
            (),
            Some(&{
                let location = PyDict::new(py);
                location.set_item("database", native_field(&entry, "database")?)?;
                location.set_item("schema", native_field(&entry, "schema")?)?;
                location.set_item("name", relation_name)?;
                location.set_item("qualified_name", qualified)?;
                location
            }),
        )?;
        entries.set_item(
            source_reads::key(&source)?,
            planner_model(
                py,
                RecordId::CloneSourcePlanEntry,
                &[
                    ("key", source_reads::key(&source)?),
                    ("name", source_reads::name(&source)?),
                    ("destination", destination),
                ],
            )?,
        )?;
    }
    in_order(&entries, execution_order)
}

fn clone_function_entries<'py>(
    (project, plan): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    adapter: &Bound<'py, PyAny>,
    resolution: &CloneResolution<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let selected = native_field(plan, "selected_keys")?;
    let entries = PyDict::new(py);
    for function in project_reads::functions(project)?.try_iter()? {
        let function = function?;
        if !selected.contains(function_reads::key(&function)?)? {
            continue;
        }
        let body_sql = resolve_function_sql(
            py,
            Some(keywords(
                py,
                &[
                    ("adapter", adapter.clone()),
                    ("function", function.clone()),
                    ("model_locations", resolution.model_locations.clone()),
                    ("seed_locations", resolution.seed_locations.clone()),
                    ("function_locations", resolution.function_locations.clone()),
                    ("source_map", resolution.source_map.clone()),
                    (
                        "source_warehouse_columns",
                        resolution.source_warehouse_columns.clone(),
                    ),
                    (
                        "star_exclude_keyword",
                        adapter.call_method0("star_exclude_keyword")?,
                    ),
                ],
            )?),
        )?;
        let field = |name: &str| native_field(&function, name);
        let entry = planner_model(
            py,
            RecordId::FunctionPlanEntry,
            &[
                ("key", field("key")?),
                ("name", field("name")?),
                ("relative_path", field("relative_path")?),
                ("destination", field("destination")?),
                ("arguments", field("arguments")?),
                ("returns", field("returns")?),
                ("body_sql", body_sql),
                (
                    "fingerprint_query_sql",
                    object(py, build_compiled_function_fingerprint_sql(&function)?)?,
                ),
                ("fingerprint_destination", field("fingerprint_destination")?),
                ("return_columns", field("return_columns")?),
                ("language", field("language")?),
                ("source_file_path", field("source_file_path")?),
                ("runtime_version", field("runtime_version")?),
                ("entry_point", field("entry_point")?),
                ("packages", field("packages")?),
            ],
        )?;
        entries.set_item(function_reads::key(&function)?, entry)?;
    }
    in_order(&entries, &native_field(plan, "execution_order")?)
}

fn seed_entry<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = seed.py();
    planner_model(
        py,
        RecordId::SeedPlanEntry,
        &[
            ("key", seed_reads::key(seed)?),
            ("name", seed_reads::name(seed)?),
            ("destination", seed_reads::destination(seed)?),
            (
                "file_path",
                native_field(&seed_reads::seed_file(seed)?, "file_path")?,
            ),
            ("columns", PyTuple::empty(py).into_any()),
            (
                "csv_settings",
                native_field(&seed_reads::schema_entry(seed)?, "csv_settings")?,
            ),
        ],
    )
}

/// Seeds whose `field` (the key or the name) is in `wanted`, as clone seed entries.
fn seed_entries<'py>(
    project: &Bound<'py, PyAny>,
    field: &str,
    wanted: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for seed in project_reads::seeds(project)?.try_iter()? {
        let seed = seed?;
        if wanted.contains(native_field(&seed, field)?)? {
            entries.push(seed_entry(&seed)?);
        }
    }
    tuple_of(project.py(), entries)
}

fn origin_model_entries<'py>(
    project: &Bound<'py, PyAny>,
    selected_names: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for model in project_reads::models(project)?.try_iter()? {
        let model = model?;
        if !selected_names.contains(model_reads::name_object(&model)?)? || disabled_model(&model)? {
            continue;
        }
        let materialization_type = materialization_of(&model)?;
        entries.push(model_entry(&model, materialization_type, string(py, ""))?);
    }
    tuple_of(py, entries)
}

fn names_of<'py>(entries: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let mut names: Vec<Bound<'py, PyAny>> = Vec::new();
    for entry in entries.try_iter()? {
        names.push(native_field(&entry?, "name")?);
    }
    Ok(PyFrozenSet::new(entries.py(), names)?.into_any())
}

fn origin_source_entries<'py>(
    project: &Bound<'py, PyAny>,
    selected_names: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let mut keys: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut order: Vec<Bound<'py, PyAny>> = Vec::new();
    for source in project_reads::sources(project)?.try_iter()? {
        let source = source?;
        if selected_names.contains(source_reads::name(&source)?)? {
            keys.push(source_reads::key(&source)?);
        }
        order.push(source_reads::key(&source)?);
    }
    clone_source_entries(
        project,
        (
            &PyFrozenSet::new(py, keys)?.into_any(),
            &tuple_of(py, order)?,
        ),
        adapter,
    )
}

/// Plan a clone, with the seconds spent inspecting sources and planning.
pub(crate) fn timed_clone_planning<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<(Bound<'py, PyTuple>, PhaseTimings)> {
    let started = std::time::Instant::now();
    let args = KeywordArguments::read(
        py,
        "timed_clone_planning",
        kwargs,
        (
            &[
                "project",
                "select",
                "exclude",
                "adapter",
                "connection",
                "origin_project",
                "origin_source_project",
                "destination_source_project",
            ],
            &[],
        ),
    )?;
    let project = args.value("project")?;
    let adapter = args.value("adapter")?;
    let connection = args.value("connection")?;
    let destination_sources = args.value("destination_source_project")?;
    let plan = clone_plan_output(&project, &args.value("select")?, &args.value("exclude")?)?;
    let inspection_started = std::time::Instant::now();
    let resolution = clone_resolution(
        (&project, &plan),
        &destination_sources,
        (&adapter, &connection),
    )?;
    let inspection_seconds = inspection_started.elapsed().as_secs_f64();
    let models = clone_model_entries((&project, &plan), &adapter, &resolution)?;
    let seeds = seed_entries(&project, "key", &native_field(&plan, "selected_keys")?)?;
    let functions = clone_function_entries((&project, &plan), &adapter, &resolution)?;
    let mut sources = PyTuple::empty(py).into_any();
    if native_field(&destination_sources, "effective_target_name")?
        .eq(native_field(&project, "effective_target_name")?)?
    {
        sources = clone_source_entries(
            &project,
            (
                &native_field(&plan, "selected_keys")?,
                &native_field(&plan, "execution_order")?,
            ),
            &adapter,
        )?;
    }
    let origin_project = args.value("origin_project")?;
    let origin_models = origin_model_entries(&origin_project, &names_of(&models)?)?;
    let origin_seeds = seed_entries(&origin_project, "name", &names_of(&seeds)?)?;
    let origin_sources = origin_source_entries(
        &args.value("origin_source_project")?,
        &names_of(&sources)?,
        &adapter,
    )?;
    let origin_by_name = PyDict::new(py);
    for entry in origin_sources.try_iter()? {
        let entry = entry?;
        origin_by_name.set_item(native_field(&entry, "name")?, entry)?;
    }
    let mut kept_sources: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut kept_origins: Vec<Bound<'py, PyAny>> = Vec::new();
    for entry in sources.try_iter()? {
        let entry = entry?;
        let Some(origin) = origin_by_name.get_item(native_field(&entry, "name")?)? else {
            continue;
        };
        if native_field(&origin, "destination")?.ne(native_field(&entry, "destination")?)? {
            kept_origins.push(origin);
            kept_sources.push(entry);
        }
    }
    let timings: PhaseTimings = vec![
        (SOURCE_INSPECTION_PHASE.to_owned(), inspection_seconds),
        (
            PLANNING_PHASE.to_owned(),
            started.elapsed().as_secs_f64() - inspection_seconds,
        ),
    ];
    let planned = PyTuple::new(
        py,
        [
            plan,
            tuple_of(py, kept_sources)?,
            models,
            seeds,
            functions,
            origin_models,
            origin_seeds,
            tuple_of(py, kept_origins)?,
        ],
    )?;
    Ok((planned, timings))
}

/// The traversal callbacks and graph of one deferred-clone query.
struct CloneGraph<'py> {
    selected: Bound<'py, PyAny>,
    upstream: Bound<'py, PyAny>,
    is_clonable: Bound<'py, PyAny>,
    is_view: Bound<'py, PyAny>,
}

impl<'py> CloneGraph<'py> {
    fn read(args: &KeywordArguments<'py>) -> PyResult<Self> {
        Ok(Self {
            selected: args.value("selected")?,
            upstream: args.value("upstream")?,
            is_clonable: args.value("is_clonable")?,
            is_view: args.value("is_view")?,
        })
    }

    fn parents(&self, node: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.upstream
            .call_method1("get", (node, PyTuple::empty(node.py())))?
            .try_iter()?
            .collect()
    }

    fn holds(&self, check: &Bound<'py, PyAny>, node: &Bound<'py, PyAny>) -> PyResult<bool> {
        check.call1((node,))?.is_truthy()
    }

    /// Depth-first walk from every selected node in Python's recursion order.
    fn walk(&self, skipped_views: bool) -> PyResult<Bound<'py, PyFrozenSet>> {
        let py = self.selected.py();
        let mut visited = PyNodeSet(PySet::empty(py)?);
        let mut found = PyNodeSet(PySet::empty(py)?);
        clone_ancestors(
            self.selected.try_iter()?,
            (&mut visited, &mut found),
            |node| self.parents(node),
            ClonePolicy {
                selected: |node: &Bound<'py, PyAny>| self.selected.contains(node),
                clonable: |node: &Bound<'py, PyAny>| self.holds(&self.is_clonable, node),
                view: |node: &Bound<'py, PyAny>| self.holds(&self.is_view, node),
                skipped_views,
            },
        )?;
        PyFrozenSet::new(py, found.0.iter())
    }
}

/// Return non-view clonable ancestors anchoring selected nodes.
#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn resolve_clone_boundary<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let args = KeywordArguments::read(
        py,
        "resolve_clone_boundary",
        kwargs,
        (&GRAPH_ARGUMENTS, &[]),
    )?;
    CloneGraph::read(&args)?.walk(false)
}

/// Return skipped view ancestors rebuilt over cloned boundaries.
#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn resolve_skipped_view_chain<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let args = KeywordArguments::read(
        py,
        "resolve_skipped_view_chain",
        kwargs,
        (&GRAPH_ARGUMENTS, &[]),
    )?;
    CloneGraph::read(&args)?.walk(true)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_clone_boundary, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_skipped_view_chain, module)?)
}
