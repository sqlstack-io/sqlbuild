//! Warehouse facts janitor planning reads through adapter callbacks, in the executor's order.

use std::collections::{BTreeSet, HashSet};

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyBool;
use sqlbuild_analysis::janitor_planning::constants::UNTRACKED_QUERY_DIFF_REASON;

use sqlbuild_host::entries::models::{AdapterHost, ExecutorHost};

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported, keywords};
use crate::bindings::_helpers::fingerprints::storage_io::read_latest_fingerprints;
use crate::bindings::_helpers::janitor::janitor_keys::{
    Folded, Key, SchemaKey, dedupe_schema_keys, folded_schema, integer, optional, record,
    sorted_schemas, text, tuple,
};
use crate::bindings::_helpers::records::fields::replaced;

const QUERY_ARTIFACT_LIFECYCLE: ExecutorHost = ExecutorHost::QueryDiffArtifactLifecycle;
const INSPECTION_LIST: AdapterHost = AdapterHost::ListRelationsForInspection;
const ACTIVE_CATALOG: AdapterHost = AdapterHost::ActiveInspectionCatalog;
const LOG_DEBUG_EVENT: ExecutorHost = ExecutorHost::LogDebugEvent;
const EXECUTION_LOGGER: &str = "sqlbuild.execution";
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::fields::field as native_field;
use sqlbuild_core::constants::FINGERPRINT_TABLE_NAME;
use sqlbuild_core::constants::{CONSUMPTION_TABLE_NAME, SOURCE_FRESHNESS_TABLE_NAME};

/// The adapter and its open connection.
pub(crate) struct Warehouse<'py> {
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) connection: Bound<'py, PyAny>,
}

impl<'py> Warehouse<'py> {
    pub(crate) fn py(&self) -> Python<'py> {
        self.adapter.py()
    }

    /// `adapter.<method>(connection=connection, **pairs)`.
    pub(crate) fn call(
        &self,
        method: &str,
        pairs: &[(&str, Bound<'py, PyAny>)],
    ) -> PyResult<Bound<'py, PyAny>> {
        let mut arguments: Vec<(&str, Bound<'py, PyAny>)> =
            vec![("connection", self.connection.clone())];
        arguments.extend(pairs.iter().cloned());
        call_with(&native_field(&self.adapter, method)?, &arguments)
    }

    /// `list_relations`, served from the open inspection catalog so each schema is read once.
    pub(crate) fn list_relations(
        &self,
        database: Option<&str>,
        (schemas, names): (Option<&[String]>, Option<&[String]>),
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = self.py();
        let texts = |values: Option<&[String]>| -> PyResult<Bound<'py, PyAny>> {
            match values {
                Some(values) => tuple(py, values.iter().map(|value| text(py, value))),
                None => Ok(py.None().into_bound(py)),
            }
        };
        call_with(
            &host::entry(py, INSPECTION_LIST)?,
            &[
                ("adapter", self.adapter.clone()),
                ("connection", self.connection.clone()),
                ("database", optional(py, database)?),
                ("schemas", texts(schemas)?),
                ("names", texts(names)?),
            ],
        )
    }

    /// The inspection catalog open on this connection, or none.
    fn catalog(&self) -> PyResult<Option<Bound<'py, PyAny>>> {
        let catalog: Bound<'py, PyAny> = call_with(
            &host::entry(self.py(), ACTIVE_CATALOG)?,
            &[
                ("adapter", self.adapter.clone()),
                ("connection", self.connection.clone()),
            ],
        )?;
        Ok((!catalog.is_none()).then_some(catalog))
    }

    /// Read every not-yet-listed schema concurrently when the open catalog lists whole schemas.
    pub(crate) fn prefetch_schema_listings(
        &self,
        scopes: &[(Option<String>, String)],
    ) -> PyResult<()> {
        let py: Python<'py> = self.py();
        let Some(catalog) = self.catalog()?.filter(|_| !scopes.is_empty()) else {
            return Ok(());
        };
        let mut items: Vec<Bound<'py, PyAny>> = Vec::new();
        for (database, schema) in scopes {
            items.push(tuple(
                py,
                [optional(py, database.as_deref())?, text(py, schema)],
            )?);
        }
        call_with(
            &native_field(&catalog, "prefetch_schema_listings")?,
            &[("scopes", tuple(py, items)?)],
        )?;
        Ok(())
    }

    /// Read these views' definitions with one read per schema, when a catalog is open.
    pub(crate) fn prefetch_view_definitions(
        &self,
        requests: &[(Option<String>, String, Vec<String>)],
    ) -> PyResult<()> {
        let py: Python<'py> = self.py();
        let Some(catalog) = self.catalog()?.filter(|_| !requests.is_empty()) else {
            return Ok(());
        };
        let mut items: Vec<Bound<'py, PyAny>> = Vec::new();
        for (database, schema, names) in requests {
            items.push(tuple(
                py,
                [
                    optional(py, database.as_deref())?,
                    text(py, schema),
                    tuple(py, names.iter().map(|name| text(py, name)))?,
                ],
            )?);
        }
        call_with(
            &native_field(&catalog, "prefetch_view_definitions")?,
            &[("requests", tuple(py, items)?)],
        )?;
        Ok(())
    }

    pub(crate) fn age_metadata_support(&self) -> PyResult<bool> {
        self.adapter
            .call_method0("supports_relation_age_metadata")?
            .is_truthy()
    }
}

/// Relations listed per target schema, in listing order.
pub(crate) type SchemaRelations<'py> = Vec<(SchemaKey, Vec<Bound<'py, PyAny>>)>;

/// Desired, discovered and tracked relation facts for planning.
pub(crate) struct WarehouseFacts<'py> {
    pub(crate) desired: HashSet<Folded>,
    pub(crate) source_schemas: Vec<(SchemaKey, BTreeSet<String>)>,
    pub(crate) relations: SchemaRelations<'py>,
    pub(crate) tracked: HashSet<Folded>,
}

impl<'py> WarehouseFacts<'py> {
    /// Relations listed for exactly this schema spelling.
    pub(crate) fn schema_relations(&self, key: &SchemaKey) -> &[Bound<'py, PyAny>] {
        self.relations
            .iter()
            .find(|(listed, _)| listed == key)
            .map_or(&[], |(_, relations)| relations.as_slice())
    }

    /// Active source names configured in one schema, compared case-insensitively.
    pub(crate) fn source_names(&self, key: &SchemaKey) -> BTreeSet<String> {
        let folded: SchemaKey = folded_schema(key);
        self.source_schemas
            .iter()
            .filter(|(candidate, _)| folded_schema(candidate) == folded)
            .flat_map(|(_, names)| names.iter().cloned())
            .collect()
    }
}

fn destination_keys<'py>(
    project: &Bound<'py, PyAny>,
    field: &str,
) -> PyResult<Vec<(Key, Bound<'py, PyAny>)>> {
    let mut keys: Vec<(Key, Bound<'py, PyAny>)> = Vec::new();
    for node in native_field(project, field)?.try_iter()? {
        let node: Bound<'py, PyAny> = node?;
        keys.push((Key::of(&native_field(&node, "destination")?)?, node));
    }
    Ok(keys)
}

/// Model destinations with their models, in project order.
pub(crate) fn model_destinations<'py>(
    project: &Bound<'py, PyAny>,
) -> PyResult<Vec<(Key, Bound<'py, PyAny>)>> {
    destination_keys(project, "models")
}

/// Schemas the compiled project writes relations to, one spelling per schema.
pub(crate) fn collect_target_schemas(project: &Bound<'_, PyAny>) -> PyResult<Vec<SchemaKey>> {
    let mut schemas: Vec<SchemaKey> = Vec::new();
    for field in ["models", "seeds"] {
        for (key, _) in destination_keys(project, field)? {
            schemas.push((key.database, key.schema));
        }
    }
    Ok(dedupe_schema_keys(schemas))
}

fn desired_keys(project: &Bound<'_, PyAny>) -> PyResult<HashSet<Folded>> {
    let mut keys: HashSet<Folded> = HashSet::new();
    for field in ["models", "seeds"] {
        for (key, _) in destination_keys(project, field)? {
            keys.insert(key.folded());
        }
    }
    Ok(keys)
}

fn source_schemas(
    project: &Bound<'_, PyAny>,
    (default_database, default_schema): (Option<String>, Option<String>),
) -> PyResult<Vec<(SchemaKey, BTreeSet<String>)>> {
    let mut schemas: Vec<(SchemaKey, BTreeSet<String>)> = Vec::new();
    for source in native_field(project, "sources")?.try_iter()? {
        let source: Bound<'_, PyAny> = source?;
        let entry: Bound<'_, PyAny> = native_field(&source, "source_entry")?;
        if !native_field(&entry, "expression")?.is_none() {
            continue;
        }
        let database: Option<String> = native_field(&entry, "database")?
            .extract::<Option<String>>()?
            .or_else(|| default_database.clone());
        let schema: Option<String> = native_field(&entry, "schema")?
            .extract::<Option<String>>()?
            .or_else(|| default_schema.clone());
        let name: String = native_field(&source, "name")?.extract()?;
        let key: SchemaKey = (database, schema);
        match schemas.iter_mut().find(|(listed, _)| *listed == key) {
            Some((_, names)) => {
                names.insert(name);
            }
            None => schemas.push((key, BTreeSet::from([name]))),
        }
    }
    Ok(schemas)
}

/// Each target schema's relations, listed once through the catalog and respelled as targeted.
fn list_target_schema_relations<'py>(
    warehouse: &Warehouse<'py>,
    target_schemas: &[SchemaKey],
) -> PyResult<SchemaRelations<'py>> {
    let py: Python<'py> = warehouse.py();
    let mut databases: Vec<Option<String>> = Vec::new();
    for (database, _) in sorted_schemas(target_schemas) {
        if !databases.contains(&database) {
            databases.push(database);
        }
    }
    warehouse.prefetch_schema_listings(&concrete_schemas(target_schemas))?;
    let mut result: SchemaRelations<'py> = Vec::new();
    for database in databases {
        let mut concrete: Vec<String> = target_schemas
            .iter()
            .filter(|(listed, _)| *listed == database)
            .filter_map(|(_, schema)| schema.clone())
            .collect();
        concrete.sort();
        concrete.dedup();
        let requests: Vec<Option<Vec<String>>> = if concrete.is_empty() {
            vec![None]
        } else {
            concrete
                .into_iter()
                .map(|schema| Some(vec![schema]))
                .collect()
        };
        for schemas in requests {
            let listed: Bound<'py, PyAny> =
                warehouse.list_relations(database.as_deref(), (schemas.as_deref(), None))?;
            for relation in listed.try_iter()? {
                let relation: Bound<'py, PyAny> = relation?;
                let folded: SchemaKey = folded_schema(&(
                    native_field(&relation, "database")?.extract()?,
                    native_field(&relation, "schema")?.extract()?,
                ));
                let Some(target) = target_schemas
                    .iter()
                    .find(|target| folded_schema(target) == folded)
                else {
                    continue;
                };
                let respelled: Bound<'py, PyAny> = replaced(
                    &relation,
                    &[
                        ("database", optional(py, target.0.as_deref())?),
                        ("schema", optional(py, target.1.as_deref())?),
                    ],
                )?;
                match result.iter_mut().find(|(listed, _)| listed == target) {
                    Some((_, relations)) => relations.push(respelled),
                    None => result.push((target.clone(), vec![respelled])),
                }
            }
        }
    }
    Ok(result)
}

/// Folded keys of relations in the locations' schemas, listed through the inspection catalog.
fn relation_lookup(
    warehouse: &Warehouse<'_>,
    locations: &[(Option<String>, String, &str)],
) -> PyResult<HashSet<Folded>> {
    let mut scopes: Vec<(Option<String>, String)> = Vec::new();
    for (database, schema, _) in locations {
        if !scopes.contains(&(database.clone(), schema.clone())) {
            scopes.push((database.clone(), schema.clone()));
        }
    }
    let mut present: HashSet<Folded> = HashSet::new();
    for (database, schema) in scopes {
        let schemas: [String; 1] = [schema];
        for relation in warehouse
            .list_relations(database.as_deref(), (Some(&schemas), None))?
            .try_iter()?
        {
            present.insert(Key::of(&relation?)?.folded());
        }
    }
    Ok(present)
}

fn lookup_exists(
    lookup: &HashSet<Folded>,
    (database, schema, name): (Option<&str>, &str, &str),
) -> bool {
    lookup.contains(
        &Key {
            database: database.map(str::to_owned),
            schema: Some(schema.to_owned()),
            name: name.to_owned(),
        }
        .folded(),
    )
}

fn concrete_schemas(target_schemas: &[SchemaKey]) -> Vec<(Option<String>, String)> {
    sorted_schemas(target_schemas)
        .into_iter()
        .filter_map(concrete_schema)
        .collect()
}

fn concrete_schema((database, schema): SchemaKey) -> Option<(Option<String>, String)> {
    Some((database, schema?))
}

/// Exact physical relation keys recorded in fingerprint metadata.
fn tracked_relation_keys(
    warehouse: &Warehouse<'_>,
    target_schemas: &[SchemaKey],
) -> PyResult<HashSet<Folded>> {
    let py: Python<'_> = warehouse.py();
    let schemas: Vec<(Option<String>, String)> = concrete_schemas(target_schemas);
    let locations: Vec<(Option<String>, String, &str)> = schemas
        .iter()
        .map(|(database, schema)| (database.clone(), schema.clone(), FINGERPRINT_TABLE_NAME))
        .collect();
    let lookup: HashSet<Folded> = relation_lookup(warehouse, &locations)?;
    let mut tracked: HashSet<Folded> = HashSet::new();
    for (database, schema) in &schemas {
        let exists: bool = lookup_exists(
            &lookup,
            (database.as_deref(), schema, FINGERPRINT_TABLE_NAME),
        );
        let read: PyResult<Bound<'_, PyAny>> = read_latest_fingerprints(Some(&keywords(
            py,
            &[
                ("connection", warehouse.connection.clone()),
                ("execute", native_field(&warehouse.adapter, "execute")?),
                (
                    "table_exists",
                    PyBool::new(py, exists).to_owned().into_any(),
                ),
                ("database", optional(py, database.as_deref())?),
                ("schema", text(py, schema)),
                (
                    "render_qualified_name",
                    native_field(&warehouse.adapter, "render_qualified_name")?,
                ),
                (
                    "render_read_latest_sql",
                    native_field(&warehouse.adapter, "render_read_latest_fingerprints_sql")?,
                ),
            ],
        )?));
        let fingerprint_set: Bound<'_, PyAny> = match read {
            Ok(set) => set,
            Err(error) if error.is_instance_of::<PyException>(py) => {
                call_with(
                    &host::entry(py, LOG_DEBUG_EVENT)?,
                    &[
                        (
                            "logger",
                            imported(py, "logging", "getLogger")?.call1((EXECUTION_LOGGER,))?,
                        ),
                        (
                            "message",
                            text(
                                py,
                                "janitor fingerprint tracking read failed; skipping schema",
                            ),
                        ),
                        ("database", optional(py, database.as_deref())?),
                        ("schema", text(py, schema)),
                        ("sqlbuild_error", error.value(py).str()?.into_any()),
                    ],
                )?;
                continue;
            }
            Err(error) => return Err(error),
        };
        for fingerprint in native_field(&fingerprint_set, "fingerprints")?
            .call_method0("values")?
            .try_iter()?
        {
            let fingerprint: Bound<'_, PyAny> = fingerprint?;
            let target_schema: Option<String> =
                native_field(&fingerprint, "target_schema")?.extract()?;
            let target_name: Option<String> =
                native_field(&fingerprint, "target_name")?.extract()?;
            let (Some(target_schema), Some(target_name)) = (target_schema, target_name) else {
                continue;
            };
            tracked.insert(
                Key {
                    database: native_field(&fingerprint, "target_database")?.extract()?,
                    schema: Some(target_schema),
                    name: target_name,
                }
                .folded(),
            );
        }
    }
    Ok(tracked)
}

/// Desired, discovered and tracked relation facts, read in the executor's call order.
pub(crate) fn gather_warehouse_facts<'py>(
    (project, warehouse): (&Bound<'py, PyAny>, &Warehouse<'py>),
    target_schemas: &[SchemaKey],
    delete_tracked_only: bool,
) -> PyResult<WarehouseFacts<'py>> {
    let desired: HashSet<Folded> = desired_keys(project)?;
    let default_database: Option<String> = warehouse
        .adapter
        .call_method0("default_database")?
        .extract()?;
    let default_schema: Option<String> = warehouse
        .adapter
        .call_method0("default_schema")?
        .extract()?;
    let source_schemas = source_schemas(project, (default_database, default_schema))?;
    let relations: SchemaRelations<'py> = list_target_schema_relations(warehouse, target_schemas)?;
    let tracked: HashSet<Folded> = if delete_tracked_only {
        tracked_relation_keys(warehouse, target_schemas)?
    } else {
        HashSet::new()
    };
    Ok(WarehouseFacts {
        desired,
        source_schemas,
        relations,
        tracked,
    })
}

/// Fingerprint, source freshness and consumption history prune candidates for target schemas.
pub(crate) fn direct_state_prune_candidates<'py>(
    warehouse: &Warehouse<'py>,
    target_schemas: &[SchemaKey],
    retain_versions: i64,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = warehouse.py();
    if retain_versions <= 0 {
        return Ok(Vec::new());
    }
    let schemas: Vec<(Option<String>, String)> = concrete_schemas(target_schemas);
    let mut locations: Vec<(Option<String>, String, &str)> = Vec::new();
    for (database, schema) in &schemas {
        locations.push((database.clone(), schema.clone(), FINGERPRINT_TABLE_NAME));
        locations.push((
            database.clone(),
            schema.clone(),
            SOURCE_FRESHNESS_TABLE_NAME,
        ));
        locations.push((database.clone(), schema.clone(), CONSUMPTION_TABLE_NAME));
    }
    let lookup: HashSet<Folded> = relation_lookup(warehouse, &locations)?;
    let mut candidates: Vec<Bound<'py, PyAny>> = Vec::new();
    for (database, schema) in &schemas {
        for (table_name, render) in [
            (
                FINGERPRINT_TABLE_NAME,
                "render_prune_fingerprint_history_sql",
            ),
            (
                SOURCE_FRESHNESS_TABLE_NAME,
                "render_prune_source_freshness_history_sql",
            ),
            (
                CONSUMPTION_TABLE_NAME,
                "render_prune_consumption_history_sql",
            ),
        ] {
            if !lookup_exists(&lookup, (database.as_deref(), schema, table_name)) {
                continue;
            }
            let prune_sql: Bound<'py, PyAny> = call_with(
                &native_field(&warehouse.adapter, render)?,
                &[
                    ("database", optional(py, database.as_deref())?),
                    ("schema", text(py, schema)),
                    ("retain_versions", integer(py, retain_versions)?),
                ],
            )?;
            candidates.push(record(
                py,
                ExecutorHost::JanitorDirectStatePruneCandidate,
                &[
                    ("database", optional(py, database.as_deref())?),
                    ("schema", text(py, schema)),
                    ("table_name", text(py, table_name)),
                    ("retain_versions", integer(py, retain_versions)?),
                    ("prune_sql", prune_sql),
                ],
            )?);
        }
    }
    Ok(candidates)
}

/// Expired query-diff artifact candidates and untracked reserved names as skipped relations.
pub(crate) type ArtifactCandidates<'py> = (Vec<Bound<'py, PyAny>>, Vec<Bound<'py, PyAny>>);

/// Expired owned query-diff artifacts, and untracked reserved names as skipped relations.
pub(crate) fn query_diff_artifact_candidates<'py>(
    warehouse: &Warehouse<'py>,
    schemas: &[(Option<String>, String)],
    now: &Bound<'py, PyAny>,
) -> PyResult<ArtifactCandidates<'py>> {
    let py: Python<'py> = warehouse.py();
    let lifecycle: Bound<'py, PyAny> = host::entry(py, QUERY_ARTIFACT_LIFECYCLE)?;
    let mut sorted: Vec<(Option<String>, String)> = schemas.to_vec();
    sorted.sort_by(|left, right| {
        (left.0.as_deref().unwrap_or(""), left.1.as_str())
            .cmp(&(right.0.as_deref().unwrap_or(""), right.1.as_str()))
    });
    let mut candidates: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut skipped: Vec<Bound<'py, PyAny>> = Vec::new();
    for (database, schema) in sorted {
        let inspection: Bound<'py, PyAny> = call_with(
            &native_field(&lifecycle, "inspect")?,
            &[
                ("adapter", warehouse.adapter.clone()),
                ("connection", warehouse.connection.clone()),
                ("database", optional(py, database.as_deref())?),
                ("schema", text(py, &schema)),
                ("now", now.clone()),
            ],
        )?;
        for artifact in native_field(&inspection, "expired")?.try_iter()? {
            let key: Key = Key::of(&artifact?)?;
            candidates.push(record(
                py,
                ExecutorHost::JanitorQueryDiffArtifactCandidate,
                &[("key", key.to_python(py)?)],
            )?);
        }
        for relation in native_field(&inspection, "untracked")?.try_iter()? {
            let relation: Bound<'py, PyAny> = relation?;
            skipped.push(skipped_relation(
                &Key::of(&relation)?,
                &relation,
                UNTRACKED_QUERY_DIFF_REASON,
            )?);
        }
    }
    Ok((candidates, skipped))
}

/// The executor's `JanitorSkippedRelation`.
pub(crate) fn skipped_relation<'py>(
    key: &Key,
    relation: &Bound<'py, PyAny>,
    reason: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = relation.py();
    record(
        py,
        ExecutorHost::JanitorSkippedRelation,
        &[
            ("key", key.to_python(py)?),
            ("relation", relation.clone()),
            ("reason", text(py, reason)),
        ],
    )
}
