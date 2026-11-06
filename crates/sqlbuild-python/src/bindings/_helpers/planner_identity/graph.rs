//! Planner execution edges, their injected origins, stable execution order and buildability over compiled keys.

use std::collections::{HashMap, HashSet};

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PySetMethods, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::main::edge_keys::edge_keys;
use sqlbuild_analysis::graph::models::EdgeDirection;
use sqlbuild_analysis::planner_identity::constants::MODEL_RESOURCE;
use sqlbuild_analysis::planner_identity::main::execution_upstream::execution_upstream;
use sqlbuild_analysis::planner_identity::main::missing_upstream_message::missing_upstream_message as message_of;
use sqlbuild_analysis::planner_identity::main::missing_upstreams::missing_upstreams;
use sqlbuild_analysis::planner_identity::main::topological_order::topological_order;
use sqlbuild_analysis::planner_identity::models::{
    AvailableUpstreams, ExecutionTest, GraphKey, MissingUpstream,
};

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::resource_reads;
use crate::bindings::_helpers::graph::algorithms::{closure, inverted};
use crate::bindings::_helpers::graph::keys::{KeyCache, native_key};
use crate::bindings::_helpers::graph::project_graph::NativeProjectGraph;
use crate::bindings::_helpers::graph::views::edge_dict;
use crate::bindings::_helpers::planner_identity::hashing::graph_key;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get_or;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use crate::bindings::_helpers::references::explicit_references::{
    GateRow, HookRow, native_execution_gate_edges,
};

const TABLE_FN_TEST_MODE: &str = "table_fn";
const INLINE_SQL_HOOK: &str = "inline SQL";

/// Python keys by graph key, keeping the first object seen for each.
struct KeyObjects<'py> {
    objects: HashMap<GraphKey, Bound<'py, PyAny>>,
}

impl<'py> KeyObjects<'py> {
    fn new() -> Self {
        Self {
            objects: HashMap::new(),
        }
    }

    fn key(&mut self, object: Bound<'py, PyAny>) -> PyResult<GraphKey> {
        let key: GraphKey = graph_key(&object)?;
        self.objects.entry(key.clone()).or_insert(object);
        Ok(key)
    }

    fn keys(&mut self, objects: &Bound<'py, PyAny>) -> PyResult<Vec<GraphKey>> {
        objects
            .try_iter()?
            .map(|object| self.key(object?))
            .collect()
    }

    fn object(&self, key: &GraphKey) -> Bound<'py, PyAny> {
        self.objects[key].clone()
    }

    fn tuple(&self, keys: &[GraphKey], py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, keys.iter().map(|key| self.object(key)))
    }
}

fn hook_label(hook_name: &str, kind: &str) -> String {
    if kind == INLINE_SQL_HOOK {
        return format!("inline SQL hook {hook_name}");
    }
    format!("{kind} hook '{hook_name}'")
}

/// Execution-order edges from lineage and woven SQL test nodes.
#[pyfunction]
pub(crate) fn build_execution_upstream_deps<'py>(
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    edge_dict(&KeyCache::new(project.py())?, &execution_deps(project)?)
}

/// Execution dependencies kept native until a public graph or plan asks for its dict view.
pub(crate) fn execution_deps(
    project: &Bound<'_, PyAny>,
) -> PyResult<Vec<(ObjectKey, Vec<ObjectKey>)>> {
    let mut resources: Vec<(ObjectKey, Vec<ObjectKey>)> = Vec::new();
    let collections: [Bound<'_, PyAny>; 4] = [
        project_reads::models(project)?,
        project_reads::sources(project)?,
        project_reads::seeds(project)?,
        project_reads::functions(project)?,
    ];
    for collection in collections {
        for resource in collection.try_iter()? {
            let row = resource_reads::graph_row(&resource?)?;
            resources.push((row.key, row.deps));
        }
    }
    let mut tests: Vec<ExecutionTest> = Vec::new();
    for test in project_reads::sql_tests(project)?.try_iter()? {
        let test = test?;
        tests.push(ExecutionTest {
            key: native_key(&native_field(&test, "key")?)?,
            scope_deps: native_field(&test, "scope_deps")?
                .try_iter()?
                .map(|key| native_key(&key?))
                .collect::<PyResult<_>>()?,
            table_function: native_field(&test, "mode")?.str()?.extract::<String>()?
                == TABLE_FN_TEST_MODE,
        });
    }
    let (gates, hooks): (Vec<GateRow>, Vec<HookRow>) = native_execution_gate_edges(project)?;
    let injected: Vec<_> = gates
        .into_iter()
        .map(|gate| (gate.gated, gate.read))
        .chain(hooks.into_iter().map(|hook| (hook.gated, hook.read)))
        .collect();
    Ok(execution_upstream(resources, &tests, &injected))
}

/// Human-readable origins for execution edges injected beyond pure lineage.
#[pyfunction]
pub(crate) fn build_execution_edge_origins<'py>(
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = project.py();
    let result = PyDict::new(py);
    let cache = KeyCache::new(py)?;
    for ((gated, read), origin) in execution_origins(project)? {
        result.set_item((cache.key(&gated)?, cache.key(&read)?), origin)?;
    }
    Ok(result)
}

/// Injected-edge descriptions remain native for cycle and buildability diagnostics.
pub(crate) fn execution_origins(
    project: &Bound<'_, PyAny>,
) -> PyResult<Vec<((ObjectKey, ObjectKey), String)>> {
    let mut origins: Vec<((ObjectKey, ObjectKey), String)> = Vec::new();
    for test in project_reads::sql_tests(project)?.try_iter()? {
        let test = test?;
        let test_name: String = native_field(&test, "name")?.extract()?;
        for target_key in native_field(&test, "scope_deps")?.try_iter()? {
            let target_key = native_key(&target_key?)?;
            let target_name = &target_key.1;
            let description = format!("SQL test '{test_name}' runs before '{target_name}'");
            origins.push((
                (target_key, native_key(&native_field(&test, "key")?)?),
                description,
            ));
        }
    }
    let (gates, hooks): (Vec<GateRow>, Vec<HookRow>) = native_execution_gate_edges(project)?;
    let lineage = native_field(project, "lineage_graph")?;
    let lineage = lineage.cast::<NativeProjectGraph>()?.borrow();
    let is_lineage = |gated: &ObjectKey, read: &ObjectKey| {
        edge_keys(lineage.graph(), gated, EdgeDirection::Upstream).contains(read)
    };
    let mut seen: HashSet<_> = origins.iter().map(|(edge, _)| edge.clone()).collect();
    for GateRow {
        audit_name,
        target,
        gated,
        read,
    } in &gates
    {
        if is_lineage(gated, read) || !seen.insert((gated.clone(), read.clone())) {
            continue;
        }
        origins.push((
            (gated.clone(), read.clone()),
            format!("audit '{audit_name}' on '{}' reads '{}'", target.1, read.1),
        ));
    }
    for HookRow {
        hook_name,
        gated,
        read,
        hook_type: kind,
        ..
    } in &hooks
    {
        if is_lineage(gated, read) || !seen.insert((gated.clone(), read.clone())) {
            continue;
        }
        origins.push((
            (gated.clone(), read.clone()),
            format!(
                "{} on '{}' reads '{}'",
                hook_label(hook_name, kind),
                gated.1,
                read.1
            ),
        ));
    }
    Ok(origins)
}

/// Stable native execution order, projecting no Python graph or key objects.
pub(crate) fn execution_order(
    py: Python<'_>,
    upstream: &[(ObjectKey, Vec<ObjectKey>)],
    origins: &[((ObjectKey, ObjectKey), String)],
) -> PyResult<Vec<ObjectKey>> {
    let key = |(kind, name): &ObjectKey| GraphKey {
        kind: kind.clone(),
        name: name.clone(),
    };
    let edges: Vec<_> = upstream
        .iter()
        .map(|(node, deps)| (key(node), deps.iter().map(key).collect()))
        .collect();
    let origins: HashMap<(GraphKey, GraphKey), String> = origins
        .iter()
        .map(|((gated, read), origin)| ((key(gated), key(read)), origin.clone()))
        .collect();
    Ok(topological_order(&edges, &origins)
        .map_err(|message| planner_input_error(py, message))?
        .into_iter()
        .map(|key| (key.kind, key.name))
        .collect())
}

/// Downstream edges keyed by upstream key, each list sorted by `(resource_type, name)`.
#[pyfunction]
pub(crate) fn build_downstream_deps<'py>(
    upstream: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = upstream.py();
    let result: Bound<'py, PyDict> = PyDict::new(py);
    for (key, sources) in inverted(upstream)?.iter() {
        let mut keyed: Vec<(GraphKey, Bound<'py, PyAny>)> = sources
            .try_iter()?
            .map(|source| {
                let source: Bound<'py, PyAny> = source?;
                Ok((graph_key(&source)?, source))
            })
            .collect::<PyResult<_>>()?;
        keyed.sort_by(|left, right| left.0.cmp(&right.0));
        result.set_item(
            key,
            PyTuple::new(py, keyed.into_iter().map(|(_, source)| source))?,
        )?;
    }
    Ok(result)
}

fn planner_input_error(py: Python<'_>, message: String) -> PyErr {
    match error_class(py, ErrorId::PlannerInput).and_then(|class| class.call1((message,))) {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

fn edge_origins<'py>(
    origins: &Bound<'py, PyAny>,
    objects: &mut KeyObjects<'py>,
) -> PyResult<HashMap<(GraphKey, GraphKey), String>> {
    let mut edges: HashMap<(GraphKey, GraphKey), String> = HashMap::new();
    if origins.is_none() {
        return Ok(edges);
    }
    for item in mapping_items(origins)?.try_iter()? {
        let ((dependent, dependency), origin): ((Bound<'py, PyAny>, Bound<'py, PyAny>), String) =
            item?.extract()?;
        edges.insert((objects.key(dependent)?, objects.key(dependency)?), origin);
    }
    Ok(edges)
}

/// All keys in stable dependency order using Kahn's algorithm.
#[pyfunction]
#[pyo3(signature = (*, upstream, injected_edge_origins = None))]
pub(crate) fn topologically_order_keys<'py>(
    upstream: &Bound<'py, PyAny>,
    injected_edge_origins: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = upstream.py();
    let mut objects = KeyObjects::new();
    let mut edges: Vec<(GraphKey, Vec<GraphKey>)> = Vec::new();
    for item in mapping_items(upstream)?.try_iter()? {
        let (key, deps): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        edges.push((objects.key(key)?, objects.keys(&deps)?));
    }
    let origins: HashMap<(GraphKey, GraphKey), String> = match injected_edge_origins {
        Some(origins) => edge_origins(&origins, &mut objects)?,
        None => HashMap::new(),
    };
    let ordered: Vec<GraphKey> =
        topological_order(&edges, &origins).map_err(|message| planner_input_error(py, message))?;
    objects.tuple(&ordered, py)
}

/// Model names reachable downstream from the given keys, including model roots.
#[pyfunction]
#[pyo3(signature = (*, start_keys, downstream_deps))]
pub(crate) fn build_downstream_model_name_closure<'py>(
    start_keys: &Bound<'py, PyAny>,
    downstream_deps: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = downstream_deps.py();
    let starts: Vec<Bound<'py, PyAny>> = start_keys.try_iter()?.collect::<PyResult<_>>()?;
    let mut names: Vec<String> = Vec::new();
    for key in closure(starts, downstream_deps, (true, None))?.iter() {
        let key: GraphKey = graph_key(&key)?;
        if key.kind == MODEL_RESOURCE {
            names.push(key.name);
        }
    }
    PyFrozenSet::new(py, names)
}

fn name_set(names: &Bound<'_, PyAny>) -> PyResult<HashSet<String>> {
    names.try_iter()?.map(|name| name?.extract()).collect()
}

fn missing_rows<'py>(
    missing: &Bound<'py, PyAny>,
    objects: &mut KeyObjects<'py>,
) -> PyResult<Vec<MissingUpstream>> {
    missing
        .try_iter()?
        .map(|entry| {
            let entry: Bound<'py, PyAny> = entry?;
            Ok(MissingUpstream {
                key: objects.key(native_field(&entry, "key")?)?,
                required_by: objects.keys(&native_field(&entry, "required_by")?)?,
            })
        })
        .collect()
}

/// Upstream deps of selected keys that exist neither in scope nor in the warehouse.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn check_buildability<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "check_buildability",
        options,
        (
            &["selected_keys", "upstream_deps", "snapshot"],
            &["deferred_relations", "satisfied_keys"],
        ),
    )?;
    let selected_keys: Bound<'py, PyAny> = arguments.value("selected_keys")?;
    let upstream_deps: Bound<'py, PyAny> = arguments.value("upstream_deps")?;
    let snapshot: Bound<'py, PyAny> = arguments.value("snapshot")?;
    let deferred_relations: Bound<'py, PyAny> = arguments.value("deferred_relations")?;
    let satisfied_keys: Bound<'py, PyAny> = arguments.value("satisfied_keys")?;
    let mut objects = KeyObjects::new();
    let selected: Vec<GraphKey> = objects.keys(&selected_keys)?;
    let mut deps: HashMap<GraphKey, Vec<GraphKey>> = HashMap::new();
    for key in &selected {
        let dep_keys: Bound<'py, PyAny> =
            mapping_get_or(&upstream_deps, objects.object(key), PyTuple::empty(py))?;
        deps.insert(key.clone(), objects.keys(&dep_keys)?);
    }
    let available = AvailableUpstreams {
        satisfied_keys: if satisfied_keys.is_none() {
            HashSet::new()
        } else {
            objects.keys(&satisfied_keys)?.into_iter().collect()
        },
        existing_relations: name_set(&native_field(&snapshot, "existing_relations")?)?,
        deferred_relations: if deferred_relations.is_none() {
            None
        } else {
            Some(name_set(&deferred_relations)?)
        },
    };
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for missing in missing_upstreams(&selected, &deps, &available) {
        entries.push(new_record_with(
            py,
            RecordId::MissingUpstream,
            &[
                ("key", objects.object(&missing.key)),
                (
                    "required_by",
                    objects.tuple(&missing.required_by, py)?.into_any(),
                ),
            ],
        )?);
    }
    PyTuple::new(py, entries)
}

/// Describe missing upstream keys, naming the audit when only an audit read requires one.
#[pyfunction]
#[pyo3(signature = (*, missing, edge_origins))]
pub(crate) fn missing_upstream_message<'py>(
    missing: &Bound<'py, PyAny>,
    edge_origins: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyString>> {
    let mut objects = KeyObjects::new();
    let rows: Vec<MissingUpstream> = missing_rows(missing, &mut objects)?;
    let origins: HashMap<(GraphKey, GraphKey), String> =
        self::edge_origins(edge_origins, &mut objects)?;
    Ok(PyString::new(missing.py(), &message_of(&rows, &origins)))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_execution_upstream_deps, module)?)?;
    module.add_function(wrap_pyfunction!(build_execution_edge_origins, module)?)?;
    module.add_function(wrap_pyfunction!(build_downstream_deps, module)?)?;
    module.add_function(wrap_pyfunction!(topologically_order_keys, module)?)?;
    module.add_function(wrap_pyfunction!(
        build_downstream_model_name_closure,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(check_buildability, module)?)?;
    module.add_function(wrap_pyfunction!(missing_upstream_message, module)?)
}
