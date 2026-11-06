//! Gather a frozen point-in-time warehouse snapshot for planning.

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::resource_reads;
use crate::bindings::_helpers::compiled_views::seed_reads;
use crate::bindings::_helpers::compiled_views::source_reads;
use crate::bindings::_helpers::fingerprints::latest_rows::select_latest_fingerprints;
use crate::bindings::_helpers::fingerprints::storage_io::read_latest_fingerprints;
use crate::bindings::_helpers::graph::keys::{KeyCache, native_key};
use crate::bindings::_helpers::model_config::typed_config::{config_node_of, model_values};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_identity::graph::execution_deps;
use crate::bindings::_helpers::planner_phases::full_refresh::effectively_full_refreshed_model_names;
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_phases::objects::{
    by_name, frozenset, model_key, native, text, tuple,
};
use crate::bindings::_helpers::planner_scope::loaders::build_upstream_intermediate_source_map;
use crate::bindings::_helpers::planner_warehouse::constants::{
    ACTIVE_CATALOG, FUNCTION_NODE_TYPES, METADATA_NAME_FILTER_LIMIT, MIGRATE_FROM_CONFIG_KEY,
    RELATIONS_MAIN_COLUMNS, RELATIONS_MAIN_LIST,
};
use crate::bindings::_helpers::planner_warehouse::cursors::{CursorInputs, cursor_snapshots};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::fields::mapping_values;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict as KeywordDict;
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple};
use sqlbuild_analysis::fingerprint_storage::constants::NODE_TYPE_MODEL;
use sqlbuild_analysis::fingerprint_storage::constants::NODE_TYPE_SEED;
use sqlbuild_analysis::graph::main::reachable_keys::reachable_keys;
use sqlbuild_core::constants::FINGERPRINT_TABLE_NAME;
use sqlbuild_core::constants::MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::OLD_NAME_VIEW_TABLE_NAME;
use sqlbuild_core::constants::SOURCE_FRESHNESS_TABLE_NAME;
use sqlbuild_model_config::types::{AuthoredNode, NodeKind};
use std::collections::{BTreeSet, HashSet};

/// Everything one snapshot gather needs.
pub(crate) struct SnapshotRequest<'py> {
    pub(crate) project: Bound<'py, PyAny>,
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) connection: Bound<'py, PyAny>,
    pub(crate) execute: Bound<'py, PyAny>,
    pub(crate) selected_keys: Bound<'py, PyAny>,
    pub(crate) full_refresh: bool,
    pub(crate) full_refresh_model_names: Bound<'py, PyAny>,
    pub(crate) on_progress: Bound<'py, PyAny>,
    pub(crate) deferred_locations: Bound<'py, PyAny>,
    pub(crate) cursor_scope: Bound<'py, PyAny>,
}

#[derive(Default)]
struct StateSchemas {
    fingerprints: BTreeSet<String>,
    freshness: BTreeSet<String>,
    old_names: BTreeSet<String>,
    migrations: BTreeSet<String>,
}

pub(crate) fn lower(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.call_method0("lower")?.extract()
}

fn names_set<'py>(py: Python<'py>, names: &BTreeSet<String>) -> PyResult<Bound<'py, PyAny>> {
    let items: Vec<Bound<'py, PyAny>> = names.iter().map(|name| text(py, name)).collect();
    frozenset(py, &items)
}

/// Nodes of the project whose destinations hold state: models, seeds and functions.
pub(crate) fn destination_nodes<'py>(
    project: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut nodes: Vec<_> = Vec::new();
    let collections: [Bound<'py, PyAny>; 3] = [
        project_reads::models(project)?,
        project_reads::seeds(project)?,
        project_reads::functions(project)?,
    ];
    for collection in collections {
        for node in collection.try_iter()? {
            nodes.push(node?);
        }
    }
    Ok(nodes)
}

fn selected_node(selected: &Bound<'_, PyAny>, node: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(selected.is_none() || selected.contains(resource_reads::key(node)?)?)
}

fn relevant_state_keys<'py>(
    project: &Bound<'py, PyAny>,
    selected: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    if selected.is_none() {
        return Ok(selected.clone());
    }
    let deps = execution_deps(project)?;
    let selected = selected
        .try_iter()?
        .map(|key| native_key(&key?))
        .collect::<PyResult<Vec<_>>>()?;
    let reachable = reachable_keys(&selected, &deps)
        .into_iter()
        .collect::<Vec<_>>();
    Ok(PyFrozenSet::new(py, KeyCache::new(py)?.keys(&reachable)?)?.into_any())
}

fn resolve_database<'py>(
    project: &Bound<'py, PyAny>,
    selected: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    for node in destination_nodes(project)? {
        if !selected_node(selected, &node)? {
            continue;
        }
        let database = native_field(&resource_reads::destination(&node)?, "database")?;
        if !database.is_none() {
            return Ok(database);
        }
    }
    native_field(project, "effective_target_database")
}

fn target_schemas(
    project: &Bound<'_, PyAny>,
    selected: &Bound<'_, PyAny>,
) -> PyResult<BTreeSet<String>> {
    let mut schemas: BTreeSet<_> = BTreeSet::new();
    for node in destination_nodes(project)? {
        if !selected_node(selected, &node)? {
            continue;
        }
        let schema = native_field(&resource_reads::destination(&node)?, "schema")?;
        if !schema.is_none() {
            schemas.insert(schema.extract()?);
        }
    }
    let fallback = native_field(project, "effective_target_schema")?;
    if schemas.is_empty() && !fallback.is_none() {
        schemas.insert(fallback.extract()?);
    }
    Ok(schemas)
}

fn source_table_name(entry: &Bound<'_, PyAny>) -> PyResult<String> {
    let table = native_field(entry, "table")?;
    if table.is_truthy()? {
        table.extract()
    } else {
        native_field(entry, "name")?.extract()
    }
}

fn model_upstream_names(
    model: &Bound<'_, PyAny>,
    (models, seeds): (&Bound<'_, PyDict>, &Bound<'_, PyDict>),
    selected_names: &HashSet<String>,
) -> PyResult<BTreeSet<String>> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for reference in model_reads::references(model)?.try_iter()? {
        let reference = reference?;
        let kind = native_field(&reference, "ref_kind")?;
        let name: String = native_field(&reference, "ref_name")?.extract()?;
        let is_ref = kind.eq("ref")?;
        if !(is_ref || kind.eq("seed")?) || selected_names.contains(&name) {
            continue;
        }
        let lookup = if is_ref { models } else { seeds };
        if let Some(node) = lookup.get_item(&name)? {
            names.insert(native_field(&native_field(&node, "destination")?, "name")?.extract()?);
        }
    }
    Ok(names)
}

fn metadata_name_filter(
    project: &Bound<'_, PyAny>,
    selected: &Bound<'_, PyAny>,
) -> PyResult<Option<BTreeSet<String>>> {
    let py = project.py();
    let mut names: BTreeSet<String> = BTreeSet::new();
    if selected.is_none() {
        for field in ["models", "seeds"] {
            for node in native_field(project, field)?.try_iter()? {
                names.insert(
                    native_field(&native_field(&node?, "destination")?, "name")?.extract()?,
                );
            }
        }
    } else {
        let selected_names: HashSet<String> = selected
            .try_iter()?
            .map(|key| native_field(&key?, "name")?.extract::<String>())
            .collect::<PyResult<_>>()?;
        let models = by_name(project, "models")?;
        let seeds = by_name(project, "seeds")?;
        let sources = PyDict::new(py);
        for source in project_reads::sources(project)?.try_iter()? {
            let entry = source_reads::source_entry(&source?)?;
            sources.set_item(native_field(&entry, "name")?, entry)?;
        }
        for key in selected.try_iter()? {
            let name = native_field(&key?, "name")?;
            if let Some(model) = models.get_item(&name)? {
                names.insert(native_field(&model_reads::destination(&model)?, "name")?.extract()?);
                names.extend(model_upstream_names(
                    &model,
                    (&models, &seeds),
                    &selected_names,
                )?);
            } else if let Some(seed) = seeds.get_item(&name)? {
                names.insert(native_field(&seed_reads::destination(&seed)?, "name")?.extract()?);
            } else if let Some(entry) = sources.get_item(&name)? {
                names.insert(source_table_name(&entry)?);
            }
        }
        let intermediate = object(
            py,
            build_upstream_intermediate_source_map(project, selected)?,
        )?;
        for entry in mapping_values(&intermediate)?.try_iter()? {
            names.insert(source_table_name(&entry?)?);
        }
    }
    if names.is_empty() || names.len() > METADATA_NAME_FILTER_LIMIT {
        return Ok(None);
    }
    for state in [
        FINGERPRINT_TABLE_NAME,
        SOURCE_FRESHNESS_TABLE_NAME,
        OLD_NAME_VIEW_TABLE_NAME,
        MIGRATION_TABLE_NAME,
    ] {
        names.insert(state.to_owned());
    }
    Ok(Some(names))
}

fn active_catalog<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host::entry(adapter.py(), ACTIVE_CATALOG)?,
        &[
            ("adapter", adapter.clone()),
            ("connection", connection.clone()),
        ],
    )
}

/// The concurrency of the active inspection catalog, one without a catalog.
pub(crate) fn inspection_concurrency(
    adapter: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
) -> PyResult<i64> {
    let catalog = active_catalog(adapter, connection)?;
    if catalog.is_none() {
        return Ok(1);
    }
    native_field(&catalog, "concurrency")?.extract()
}

fn prefetch_schemas<'py>(
    request: &SnapshotRequest<'py>,
    database: &Bound<'py, PyAny>,
    schemas: &BTreeSet<String>,
    relevant: &Bound<'py, PyAny>,
) -> PyResult<()> {
    let py = request.project.py();
    let catalog = active_catalog(&request.adapter, &request.connection)?;
    if catalog.is_none() {
        return Ok(());
    }
    let mut scopes: Vec<Bound<'py, PyAny>> = Vec::new();
    for schema in schemas {
        scopes.push(PyTuple::new(py, [database.clone(), text(py, schema)])?.into_any());
    }
    for source in project_reads::sources(&request.project)?.try_iter()? {
        let source = source?;
        let entry = source_reads::source_entry(&source)?;
        if (!relevant.is_none() && !relevant.contains(source_reads::key(&source)?)?)
            || !native_field(&entry, "expression")?.is_none()
            || native_field(&entry, "schema")?.is_none()
        {
            continue;
        }
        scopes.push(
            PyTuple::new(
                py,
                [
                    native_field(&entry, "database")?,
                    native_field(&entry, "schema")?,
                ],
            )?
            .into_any(),
        );
    }
    call_with(
        &native_field(&catalog, "prefetch_schema_listings")?,
        &[
            ("scopes", tuple(py, scopes)?),
            ("best_effort", true.into_pyobject(py)?.to_owned().into_any()),
        ],
    )?;
    Ok(())
}

/// List relations through the inspection catalog wrappers.
pub(crate) fn list_relations<'py>(
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    database: &Bound<'py, PyAny>,
    schemas: Bound<'py, PyAny>,
    names: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host::entry(adapter.py(), RELATIONS_MAIN_LIST)?,
        &[
            ("adapter", adapter.clone()),
            ("connection", connection.clone()),
            ("database", database.clone()),
            ("schemas", schemas),
            ("names", names),
        ],
    )
}

/// Columns of the relations through the inspection catalog wrappers.
pub(crate) fn columns_for<'py>(
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    relations: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host::entry(adapter.py(), RELATIONS_MAIN_COLUMNS)?,
        &[
            ("adapter", adapter.clone()),
            ("connection", connection.clone()),
            ("relations", relations),
        ],
    )
}

fn optional_names<'py>(
    py: Python<'py>,
    names: Option<&BTreeSet<String>>,
) -> PyResult<Bound<'py, PyAny>> {
    match names {
        None => Ok(py.None().into_bound(py)),
        Some(names) => tuple(py, names.iter().map(|name| text(py, name)).collect()),
    }
}

fn location_identity<'py>(location: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = location.py();
    let part = |name: &str| -> PyResult<Bound<'py, PyAny>> {
        let value = native_field(location, name)?;
        if value.is_none() {
            Ok(value)
        } else {
            value.call_method0("lower")
        }
    };
    Ok(PyTuple::new(py, [part("database")?, part("schema")?, part("name")?])?.into_any())
}

fn gather_relations<'py>(
    request: &SnapshotRequest<'py>,
    database: &Bound<'py, PyAny>,
    schemas: Bound<'py, PyAny>,
    names: Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyDict>, StateSchemas)> {
    let py = request.project.py();
    let relations = list_relations(
        (&request.adapter, &request.connection),
        database,
        schemas,
        names,
    )?;
    let logical = PyDict::new(py);
    for node in destination_nodes(&request.project)? {
        logical.set_item(
            location_identity(&resource_reads::destination(&node)?)?,
            resource_reads::name(&node)?,
        )?;
    }
    let result = PyDict::new(py);
    let mut states = StateSchemas::default();
    for relation in relations.try_iter()? {
        let relation = relation?;
        let name: String = native_field(&relation, "name")?.extract()?;
        let schema = native_field(&relation, "schema")?;
        let schema = if schema.is_none() {
            None
        } else {
            Some(lower(&schema)?)
        };
        if name == MIGRATION_TABLE_NAME
            && let Some(schema) = &schema
        {
            states.migrations.insert(schema.clone());
        }
        let state = if name == OLD_NAME_VIEW_TABLE_NAME {
            Some(&mut states.old_names)
        } else if name == FINGERPRINT_TABLE_NAME {
            Some(&mut states.fingerprints)
        } else if name == SOURCE_FRESHNESS_TABLE_NAME {
            Some(&mut states.freshness)
        } else {
            None
        };
        if let Some(state) = state {
            if let Some(schema) = schema {
                state.insert(schema);
            }
            continue;
        }
        let identity = native_field(&relation, "identity")?;
        let key = logical
            .get_item(&identity)?
            .unwrap_or_else(|| PyString::new(py, &name).into_any());
        result.set_item(key, relation)?;
    }
    Ok((result, states))
}

fn gather_columns<'py>(
    request: &SnapshotRequest<'py>,
    relations: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = request.project.py();
    let mut node_names: HashSet<String> = HashSet::new();
    for node in destination_nodes(&request.project)? {
        node_names.insert(resource_reads::name(&node)?.extract()?);
    }
    let mut needed: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)> = Vec::new();
    for (logical, relation) in relations.iter() {
        if node_names.contains(&logical.extract::<String>()?) {
            needed.push((logical, relation));
        }
    }
    let result = PyDict::new(py);
    if needed.is_empty() {
        return Ok(result);
    }
    let all = columns_for(
        (&request.adapter, &request.connection),
        tuple(
            py,
            needed
                .iter()
                .map(|(_, relation)| relation.clone())
                .collect(),
        )?,
    )?;
    for (logical, relation) in needed {
        let identity = native_field(&relation, "identity")?;
        if all.contains(&identity)? {
            result.set_item(logical, all.get_item(identity)?)?;
        }
    }
    Ok(result)
}

fn shares_unfiltered(project: &Bound<'_, PyAny>, selected: &Bound<'_, PyAny>) -> PyResult<bool> {
    if selected.is_none() {
        return Ok(true);
    }
    let names: HashSet<String> = selected
        .try_iter()?
        .map(|key| native_field(&key?, "name")?.extract::<String>())
        .collect::<PyResult<_>>()?;
    let mut all_selected = true;
    let mut any_migration = false;
    for model in project_reads::models(project)?.try_iter()? {
        let model = model?;
        if !names.contains(&model_reads::model_name(&model)?) {
            all_selected = false;
        }
        if config_node_of(&model_values(&model)?, MIGRATE_FROM_CONFIG_KEY)?.kind() == NodeKind::Str
        {
            any_migration = true;
        }
    }
    Ok(all_selected || any_migration)
}

fn gather_fingerprints<'py>(
    request: &SnapshotRequest<'py>,
    database: &Bound<'py, PyAny>,
    schemas: Option<&BTreeSet<String>>,
    (state_schemas, relevant): (&BTreeSet<String>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py = request.project.py();
    let Some(schemas) = schemas else {
        return native(py, RecordId::WarehouseFingerprints, &[]);
    };
    let node_names: Option<BTreeSet<String>> = if relevant.is_none() {
        None
    } else {
        Some(
            relevant
                .try_iter()?
                .map(|key| native_field(&key?, "name")?.extract::<String>())
                .collect::<PyResult<_>>()?,
        )
    };
    let node_names_value = optional_names(py, node_names.as_ref())?;
    let read_unfiltered = shares_unfiltered(&request.project, relevant)?;
    let models = PyDict::new(py);
    let functions = PyDict::new(py);
    let seeds = PyDict::new(py);
    let python_nodes = PyDict::new(py);
    let unfiltered = PyDict::new(py);
    let filtered_types = tuple(
        py,
        [
            NODE_TYPE_MODEL,
            FUNCTION_NODE_TYPES[0],
            FUNCTION_NODE_TYPES[1],
            NODE_TYPE_SEED,
        ]
        .iter()
        .map(|name| text(py, name))
        .collect(),
    )?;
    for schema in schemas {
        let schema_lower = lower(&text(py, schema))?;
        let mut set = read_latest_fingerprints(Some(&keywords(
            py,
            &[
                ("connection", request.connection.clone()),
                ("execute", request.execute.clone()),
                (
                    "table_exists",
                    state_schemas
                        .contains(&schema_lower)
                        .into_pyobject(py)?
                        .to_owned()
                        .into_any(),
                ),
                ("database", database.clone()),
                ("schema", text(py, schema)),
                (
                    "render_qualified_name",
                    native_field(&request.adapter, "render_qualified_name")?,
                ),
                (
                    "render_read_latest_sql",
                    native_field(&request.adapter, "render_read_latest_fingerprints_sql")?,
                ),
                (
                    "node_names",
                    if read_unfiltered {
                        py.None().into_bound(py)
                    } else {
                        node_names_value.clone()
                    },
                ),
                ("filtered_node_types", filtered_types.clone()),
            ],
        )?))?;
        if read_unfiltered {
            let by_identity = native_field(&set, "fingerprints_by_identity")?;
            let values: Vec<Bound<'py, PyAny>> = if by_identity.is_none() {
                Vec::new()
            } else {
                mapping_values(&by_identity)?
                    .try_iter()?
                    .collect::<PyResult<_>>()?
            };
            unfiltered.set_item(&schema_lower, tuple(py, values)?)?;
            if node_names.is_some() {
                set = select_latest_fingerprints(
                    py,
                    &set,
                    node_names_value.extract::<Vec<String>>()?,
                    filtered_types.extract::<Vec<String>>()?,
                )?;
            }
        }
        for (name, fingerprint) in native_field(&set, "fingerprints")?
            .downcast::<PyDict>()?
            .iter()
        {
            let node_type: String = native_field(&fingerprint, "node_type")?.extract()?;
            if node_type == NODE_TYPE_MODEL {
                models.set_item(name, fingerprint)?;
            } else if FUNCTION_NODE_TYPES.contains(&node_type.as_str()) {
                functions.set_item(name, fingerprint)?;
            } else if node_type == NODE_TYPE_SEED {
                seeds.set_item(name, fingerprint)?;
            }
        }
        let by_identity = native_field(&set, "fingerprints_by_identity")?;
        if !by_identity.is_none() {
            for item in mapping_items(&by_identity)?.try_iter()? {
                let (key, fingerprint): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
                let node_type: String = native_field(&fingerprint, "node_type")?.extract()?;
                if node_type != NODE_TYPE_MODEL
                    && node_type != NODE_TYPE_SEED
                    && !FUNCTION_NODE_TYPES.contains(&node_type.as_str())
                {
                    python_nodes.set_item(key, fingerprint)?;
                }
            }
        }
    }
    native(
        py,
        RecordId::WarehouseFingerprints,
        &[
            ("models", models.into_any()),
            ("functions", functions.into_any()),
            ("seeds", seeds.into_any()),
            ("python_nodes", python_nodes.into_any()),
            ("unfiltered_schemas", unfiltered.into_any()),
            ("unfiltered_database", database.clone()),
        ],
    )
}

fn scope_field<'py>(scope: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    if scope.is_none() {
        Ok(scope.clone())
    } else {
        native_field(scope, name)
    }
}

/// Gather relations, columns, fingerprints and cursor bounds of the planned target schemas.
pub(crate) fn warehouse_snapshot<'py>(
    request: &SnapshotRequest<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = request.project.py();
    let project = &request.project;
    let relevant = relevant_state_keys(project, &request.selected_keys)?;
    let database = resolve_database(project, &relevant)?;
    let schemas = target_schemas(project, &relevant)?;
    let metadata_names = metadata_name_filter(project, &relevant)?;
    if schemas.is_empty() && metadata_names.is_none() {
        return native(py, RecordId::WarehouseSnapshot, &[]);
    }
    let query_schemas = (!schemas.is_empty()).then_some(&schemas);
    prefetch_schemas(request, &database, &schemas, &relevant)?;
    let (relations, states) = gather_relations(
        request,
        &database,
        optional_names(py, query_schemas)?,
        optional_names(py, metadata_names.as_ref())?,
    )?;
    let columns = gather_columns(request, &relations)?;
    let fingerprints = gather_fingerprints(
        request,
        &database,
        query_schemas,
        (&states.fingerprints, &relevant),
    )?;
    let full_refresh_names = if request.full_refresh_model_names.is_none() {
        effectively_full_refreshed_model_names(project, request.full_refresh)?
    } else {
        request.full_refresh_model_names.clone()
    };
    let scope = &request.cursor_scope;
    let cursor_selected = if scope.is_none() {
        request.selected_keys.clone()
    } else {
        native_field(scope, "model_keys")?
    };
    let cursors = cursor_snapshots(
        request,
        &relations,
        &CursorInputs {
            selected_keys: cursor_selected,
            full_refresh_model_names: full_refresh_names,
            deferred_locations: request.deferred_locations.clone(),
            runtime_producer_keys: scope_field(scope, "runtime_producer_keys")?,
            invocation_time: scope_field(scope, "invocation_time")?,
            start_cursor_config: scope_field(scope, "start_cursor_config")?,
            cursor_overrides: scope_field(scope, "cursor_overrides")?,
            target_relation_overrides: None,
            origin_cursor_column_overrides: None,
            existing_columns: Some(columns.clone()),
        },
    )?;
    let listed = match query_schemas {
        None => py.None().into_bound(py),
        Some(schemas) => {
            let mut lowered: BTreeSet<_> = BTreeSet::new();
            for schema in schemas {
                lowered.insert(lower(&text(py, schema))?);
            }
            names_set(py, &lowered)?
        }
    };
    native(
        py,
        RecordId::WarehouseSnapshot,
        &[
            ("existing_relations", relations.into_any()),
            ("existing_columns", columns.into_any()),
            ("fingerprints", fingerprints),
            ("cursor_snapshots", cursors.into_any()),
            (
                "source_freshness_state_schemas",
                names_set(py, &states.freshness)?,
            ),
            (
                "column_dialect",
                request.adapter.call_method0("sql_analysis_dialect")?,
            ),
            (
                "old_name_view_state_schemas",
                names_set(py, &states.old_names)?,
            ),
            (
                "migration_state_schemas",
                names_set(py, &states.migrations)?,
            ),
            ("listed_state_schemas", listed),
        ],
    )
}

#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn gather_warehouse_snapshot<'py>(
    py: pyo3::Python<'py>,
    options: Option<Bound<'py, KeywordDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "gather_warehouse_snapshot",
        options,
        (
            &["project", "adapter", "connection", "execute"],
            &[
                "selected_keys",
                "full_refresh",
                "full_refresh_model_names",
                "on_progress",
                "deferred_locations",
                "cursor_scope",
            ],
        ),
    )?;
    let project = &args.value("project")?;
    let adapter = &args.value("adapter")?;
    let connection = &args.value("connection")?;
    let execute = &args.value("execute")?;
    let selected_keys = &args.value("selected_keys")?;
    let full_refresh: bool = args.flag("full_refresh")?;
    let full_refresh_model_names = &args.value("full_refresh_model_names")?;
    let on_progress = &args.value("on_progress")?;
    let deferred_locations = &args.value("deferred_locations")?;
    let cursor_scope = &args.value("cursor_scope")?;

    warehouse_snapshot(&SnapshotRequest {
        project: project.clone(),
        adapter: adapter.clone(),
        connection: connection.clone(),
        execute: execute.clone(),
        selected_keys: selected_keys.clone(),
        full_refresh,
        full_refresh_model_names: full_refresh_model_names.clone(),
        on_progress: on_progress.clone(),
        deferred_locations: deferred_locations.clone(),
        cursor_scope: cursor_scope.clone(),
    })
}

/// Cursor snapshots of models whose history lives in another relation or column.
pub(crate) fn gather_redirected_cursor_snapshots<'py>(
    py: pyo3::Python<'py>,
    options: Option<Bound<'py, KeywordDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "gather_redirected_cursor_snapshots",
        options,
        (
            &[
                "project",
                "adapter",
                "connection",
                "existing_relations",
                "target_relations",
                "cursor_scope",
                "full_refresh_model_names",
            ],
            &["deferred_locations", "on_progress", "origin_cursor_columns"],
        ),
    )?;
    let project = &args.value("project")?;
    let adapter = &args.value("adapter")?;
    let connection = &args.value("connection")?;
    let existing_relations = args.value("existing_relations")?;
    let existing_relations = existing_relations.downcast::<PyDict>()?;
    let target_relations = args.value("target_relations")?.downcast_into::<PyDict>()?;
    let cursor_scope = &args.value("cursor_scope")?;
    let full_refresh_model_names = &args.value("full_refresh_model_names")?;
    let deferred_locations = &args.value("deferred_locations")?;
    let origin_cursor_columns = args.value("origin_cursor_columns")?;
    let result = PyDict::new(py);
    if target_relations.is_empty() {
        return Ok(result.into_any());
    }
    let mut keys: Vec<Bound<'py, PyAny>> = Vec::new();
    for name in target_relations.keys() {
        keys.push(model_key(py, &name)?);
    }
    let request = SnapshotRequest {
        project: project.clone(),
        adapter: adapter.clone(),
        connection: connection.clone(),
        execute: native_field(adapter, "execute")?,
        selected_keys: frozenset(py, &keys)?,
        full_refresh: false,
        full_refresh_model_names: full_refresh_model_names.clone(),
        on_progress: args.value("on_progress")?,
        deferred_locations: deferred_locations.clone(),
        cursor_scope: cursor_scope.clone(),
    };
    let snapshots = cursor_snapshots(
        &request,
        existing_relations,
        &CursorInputs {
            selected_keys: frozenset(py, &keys)?,
            full_refresh_model_names: full_refresh_model_names.clone(),
            deferred_locations: deferred_locations.clone(),
            runtime_producer_keys: native_field(cursor_scope, "runtime_producer_keys")?,
            invocation_time: native_field(cursor_scope, "invocation_time")?,
            start_cursor_config: native_field(cursor_scope, "start_cursor_config")?,
            cursor_overrides: native_field(cursor_scope, "cursor_overrides")?,
            target_relation_overrides: Some(target_relations),
            origin_cursor_column_overrides: if origin_cursor_columns.is_none() {
                None
            } else {
                Some(origin_cursor_columns.downcast_into::<PyDict>()?)
            },
            existing_columns: None,
        },
    )?;
    let models = by_name(project, "models")?;
    for (name, snapshot) in snapshots.iter() {
        if let Some(model) = models.get_item(&name)? {
            result.set_item(
                name,
                crate::bindings::_helpers::planner_phases::objects::replace(
                    &snapshot,
                    &[(
                        "target_relation",
                        native_field(&model_reads::destination(&model)?, "qualified_name")?,
                    )],
                )?,
            )?;
        }
    }
    Ok(result.into_any())
}

/// The relation names one metadata listing is filtered to, or `None` for an unfiltered listing.
#[pyfunction]
#[pyo3(signature = (*, project, selected_keys=None))]
pub(crate) fn planner_metadata_name_filter<'py>(
    py: Python<'py>,
    project: &Bound<'py, PyAny>,
    selected_keys: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let selected = selected_keys.unwrap_or_else(|| py.None().into_bound(py));
    optional_names(py, metadata_name_filter(project, &selected)?.as_ref())
}
