//! Gather a frozen point-in-time warehouse snapshot for planning.

use crate::planner_phases::main::rebuilds::effectively_full_refreshed_model_names::effectively_full_refreshed_model_names;
use crate::planner_phases::models::FullRefreshRequest;
use crate::planner_warehouse::_helpers::inspection::cursors::{CursorInputs, cursor_snapshots};
use crate::planner_warehouse::constants::{
    FUNCTION_NODE_TYPES, METADATA_NAME_FILTER_LIMIT, MIGRATE_FROM_CONFIG_KEY,
};
use crate::planner_warehouse::models::FingerprintRead;
use crate::planner_warehouse::models::PlannerReads;
use crate::planner_warehouse::models::SnapshotRequest;
use pyo3::IntoPyObjectExt;
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
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::native_host::types::Syntax;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::main::fields::mapping_values::mapping_values;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::model_config::main::config::config_node_of::config_node_of;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_name::model_name;
use sqlbuild_python_project::compiled_views::main::compiled_models::references::references;
use sqlbuild_python_project::compiled_views::main::project::compiled_models::compiled_models;
use sqlbuild_python_project::fingerprints::main::select_latest_fingerprints::select_latest_fingerprints;
use sqlbuild_python_project::graph::main::nodes::native_key::native_key;
use sqlbuild_python_project::graph::models::KeyCache;
use sqlbuild_python_project::planner_identity::main::execution_deps::execution_deps;
use sqlbuild_python_project::planner_phases::main::by_name::by_name;
use sqlbuild_python_project::planner_phases::main::frozenset::frozenset;
use sqlbuild_python_project::planner_phases::main::model_key::model_key;
use sqlbuild_python_project::planner_phases::main::native::native;
use sqlbuild_python_project::planner_phases::main::object::object;
use sqlbuild_python_project::planner_phases::main::text::text;
use sqlbuild_python_project::planner_phases::main::tuple::tuple;
use sqlbuild_python_project::planner_scope::main::build_upstream_intermediate_source_map::build_upstream_intermediate_source_map;
use sqlbuild_python_project::resource_views::main::source_reads::source_entry::source_entry;
use std::collections::{BTreeSet, HashSet};

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
        compiled_models(project)?,
        sqlbuild_python_project::compiled_views::main::project::seeds::seeds(project)?,
        sqlbuild_python_project::compiled_views::main::project::functions::functions(project)?,
    ];
    for collection in collections {
        for node in collection.try_iter()? {
            nodes.push(node?);
        }
    }
    Ok(nodes)
}

fn selected_node(selected: &Bound<'_, PyAny>, node: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(selected.is_none()
        || selected.contains(
            sqlbuild_python_project::resource_views::main::resource_reads::key::key(node)?,
        )?)
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
        let database = native_field(&sqlbuild_python_project::resource_views::main::resource_reads::destination::destination(&node)?, "database")?;
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
        let schema = native_field(&sqlbuild_python_project::resource_views::main::resource_reads::destination::destination(&node)?, "schema")?;
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
    for reference in references(model)?.try_iter()? {
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
        for source in
            sqlbuild_python_project::compiled_views::main::project::sources::sources(project)?
                .try_iter()?
        {
            let entry = source_entry(&source?)?;
            sources.set_item(native_field(&entry, "name")?, entry)?;
        }
        for key in selected.try_iter()? {
            let name = native_field(&key?, "name")?;
            if let Some(model) = models.get_item(&name)? {
                names.insert(native_field(&sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(&model)?, "name")?.extract()?);
                names.extend(model_upstream_names(
                    &model,
                    (&models, &seeds),
                    &selected_names,
                )?);
            } else if let Some(seed) = seeds.get_item(&name)? {
                names.insert(native_field(&sqlbuild_python_project::resource_views::main::seed_reads::destination::destination(&seed)?, "name")?.extract()?);
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

fn prefetch_schemas<'py>(
    request: &SnapshotRequest<'py>,
    database: &Bound<'py, PyAny>,
    schemas: &BTreeSet<String>,
    relevant: &Bound<'py, PyAny>,
) -> PyResult<()> {
    let py = request.project.py();
    let mut scopes: Vec<Bound<'py, PyAny>> = Vec::new();
    for schema in schemas {
        scopes.push(PyTuple::new(py, [database.clone(), text(py, schema)])?.into_any());
    }
    for source in
        sqlbuild_python_project::compiled_views::main::project::sources::sources(&request.project)?
            .try_iter()?
    {
        let source = source?;
        let entry = source_entry(&source)?;
        if (!relevant.is_none()
            && !relevant.contains(
                sqlbuild_python_project::resource_views::main::source_reads::key::key(&source)?,
            )?)
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
    request
        .reads
        .get()
        .reads()
        .prefetch_schema_listings(py, tuple(py, scopes)?)
}

/// The listed relations of `database` narrowed to `schemas` and `names`.
pub(crate) fn list_relations<'py>(
    reads: &Bound<'py, PlannerReads>,
    database: &Bound<'py, PyAny>,
    schemas: Bound<'py, PyAny>,
    names: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    reads
        .get()
        .reads()
        .relations(reads.py(), database, (schemas, names))
}

/// The columns of each listed relation, keyed by identity.
pub(crate) fn columns_for<'py>(
    reads: &Bound<'py, PlannerReads>,
    relations: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    reads.get().reads().columns(reads.py(), relations)
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
    let relations = list_relations(&request.reads, database, schemas, names)?;
    let logical = PyDict::new(py);
    for node in destination_nodes(&request.project)? {
        logical.set_item(
            location_identity(&sqlbuild_python_project::resource_views::main::resource_reads::destination::destination(&node)?)?,
            sqlbuild_python_project::resource_views::main::resource_reads::name::name(&node)?,
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

/// Listed columns of `relevant` nodes, the selection and the upstream its warnings check; else all.
fn gather_columns<'py>(
    request: &SnapshotRequest<'py>,
    relations: &Bound<'py, PyDict>,
    relevant: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = request.project.py();
    let mut node_names: HashSet<String> = HashSet::new();
    for node in destination_nodes(&request.project)? {
        if selected_node(relevant, &node)? {
            node_names.insert(
                sqlbuild_python_project::resource_views::main::resource_reads::name::name(&node)?
                    .extract()?,
            );
        }
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
        &request.reads,
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
    for model in compiled_models(project)?.try_iter()? {
        let model = model?;
        if !names.contains(&model_name(&model)?) {
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
    let location = node_results_location(request, database, schemas)?;
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
        let table_exists: bool = state_schemas.contains(&schema_lower);
        let read_names: Option<Vec<String>> = if read_unfiltered {
            None
        } else {
            node_names_value.extract()?
        };
        let mut set = request.reads.get().reads().fingerprints(
            py,
            FingerprintRead {
                database: database.extract()?,
                schema,
                table_exists,
                node_names: read_names,
                filtered_node_types: filtered_types.extract()?,
            },
        )?;
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
        for (name, fingerprint) in native_field(&set, "fingerprints")?.cast::<PyDict>()?.iter() {
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
        let holds_python_nodes = match &location {
            None => true,
            Some(location) => location.read_as.as_deref() == Some(schema_lower.as_str()),
        };
        if holds_python_nodes && !by_identity.is_none() {
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
    if let Some(location) = location.filter(|location| location.read_as.is_none()) {
        read_node_result_fingerprints(request, location, &python_nodes)?;
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

/// Where the Python nodes of a plan record their fingerprints.
struct NodeResultsLocation {
    database: Option<String>,
    schema: String,
    /// The lowercased schema when the snapshot reads it in its own database.
    read_as: Option<String>,
}

/// The request's node results location, or `None` when it has none.
fn node_results_location(
    request: &SnapshotRequest<'_>,
    database: &Bound<'_, PyAny>,
    schemas: &BTreeSet<String>,
) -> PyResult<Option<NodeResultsLocation>> {
    let py = request.project.py();
    let Some((node_database, Some(schema))) = request.node_results_location.clone() else {
        return Ok(None);
    };
    let schema_lower = lower(&text(py, &schema))?;
    let same_database = match (&node_database, database.is_none()) {
        (None, true) => true,
        (Some(name), false) => lower(&text(py, name))? == lower(database)?,
        _ => false,
    };
    let mut read_as = None;
    for name in schemas {
        if same_database && lower(&text(py, name))? == schema_lower {
            read_as = Some(schema_lower.clone());
        }
    }
    Ok(Some(NodeResultsLocation {
        database: node_database,
        schema,
        read_as,
    }))
}

/// Add Python node fingerprints from an unread results location, listing only its fingerprints table.
fn read_node_result_fingerprints<'py>(
    request: &SnapshotRequest<'py>,
    NodeResultsLocation {
        database, schema, ..
    }: NodeResultsLocation,
    python_nodes: &Bound<'py, PyDict>,
) -> PyResult<()> {
    let py = request.project.py();
    let node_database: Bound<'py, PyAny> = database.clone().into_bound_py_any(py)?;
    let node_schema: Bound<'py, PyAny> = text(py, &schema);
    let mut table_exists = false;
    for relation in list_relations(
        &request.reads,
        &node_database,
        tuple(py, vec![node_schema.clone()])?,
        tuple(py, vec![text(py, FINGERPRINT_TABLE_NAME)])?,
    )?
    .try_iter()?
    {
        let name: String = native_field(&relation?, "name")?.extract()?;
        table_exists |= name.eq_ignore_ascii_case(FINGERPRINT_TABLE_NAME);
    }
    let set = request.reads.get().reads().fingerprints(
        py,
        FingerprintRead {
            database,
            schema: &schema,
            table_exists,
            node_names: Some(Vec::new()),
            filtered_node_types: [
                NODE_TYPE_MODEL,
                FUNCTION_NODE_TYPES[0],
                FUNCTION_NODE_TYPES[1],
                NODE_TYPE_SEED,
            ]
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
        },
    )?;
    let by_identity = native_field(&set, "fingerprints_by_identity")?;
    if by_identity.is_none() {
        return Ok(());
    }
    for item in mapping_items(&by_identity)?.try_iter()? {
        let (key, fingerprint): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        python_nodes.set_item(key, fingerprint)?;
    }
    Ok(())
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
    let columns = gather_columns(request, &relations, &relevant)?;
    let fingerprints = gather_fingerprints(
        request,
        &database,
        query_schemas,
        (&states.fingerprints, &relevant),
    )?;
    let full_refresh_names = if request.full_refresh_model_names.is_none() {
        effectively_full_refreshed_model_names(
            project,
            &FullRefreshRequest {
                all: request.full_refresh,
                ..FullRefreshRequest::default()
            },
        )?
    } else {
        request.full_refresh_model_names.clone()
    };
    let scope = &request.cursor_scope;
    let cursor_selected = if scope.is_none() {
        request.selected_keys.clone()
    } else {
        native_field(scope, "model_keys")?
    };
    let (cursors, cursor_inputs_at_run) = cursor_snapshots(
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
                "cursor_inputs_at_run",
                pyo3::types::PyBool::new(py, cursor_inputs_at_run)
                    .to_owned()
                    .into_any(),
            ),
            (
                "source_freshness_state_schemas",
                names_set(py, &states.freshness)?,
            ),
            (
                "column_dialect",
                request.dialect.syntax("sql_analysis_dialect")?.call0()?,
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
            &["project", "dialect", "reads"],
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
    let dialect = Dialect::passed(args.value("dialect")?)?;
    let reads = PlannerReads::passed(args.value("reads")?)?;
    let selected_keys = &args.value("selected_keys")?;
    let full_refresh: bool = args.flag("full_refresh")?;
    let full_refresh_model_names = &args.value("full_refresh_model_names")?;
    let on_progress = &args.value("on_progress")?;
    let deferred_locations = &args.value("deferred_locations")?;
    let cursor_scope = &args.value("cursor_scope")?;

    warehouse_snapshot(&SnapshotRequest {
        project: project.clone(),
        dialect,
        reads,
        selected_keys: selected_keys.clone(),
        full_refresh,
        full_refresh_model_names: full_refresh_model_names.clone(),
        on_progress: on_progress.clone(),
        deferred_locations: deferred_locations.clone(),
        cursor_scope: cursor_scope.clone(),
        node_results_location: None,
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
                "dialect",
                "reads",
                "existing_relations",
                "target_relations",
                "cursor_scope",
                "full_refresh_model_names",
            ],
            &["deferred_locations", "on_progress", "origin_cursor_columns"],
        ),
    )?;
    let project = &args.value("project")?;
    let dialect = Dialect::passed(args.value("dialect")?)?;
    let reads = PlannerReads::passed(args.value("reads")?)?;
    let existing_relations = args.value("existing_relations")?;
    let existing_relations = existing_relations.cast::<PyDict>()?;
    let target_relations = args.value("target_relations")?.cast_into::<PyDict>()?;
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
        dialect,
        reads,
        selected_keys: frozenset(py, &keys)?,
        full_refresh: false,
        full_refresh_model_names: full_refresh_model_names.clone(),
        on_progress: args.value("on_progress")?,
        deferred_locations: deferred_locations.clone(),
        cursor_scope: cursor_scope.clone(),
        node_results_location: None,
    };
    let (snapshots, _) = cursor_snapshots(
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
                Some(origin_cursor_columns.cast_into::<PyDict>()?)
            },
            existing_columns: None,
        },
    )?;
    let models = by_name(project, "models")?;
    for (name, snapshot) in snapshots.iter() {
        if let Some(model) = models.get_item(&name)? {
            result.set_item(
                name,
                sqlbuild_python_project::planner_phases::main::replace::replace(
                    &snapshot,
                    &[(
                        "target_relation",
                        native_field(&sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(&model)?, "qualified_name")?,
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
