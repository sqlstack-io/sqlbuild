//! The janitor cleanup plan: warehouse facts through adapter callbacks, decisions in Rust.

use std::collections::HashSet;

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyDict, PyDictMethods};
use sqlbuild_analysis::janitor_planning::constants::OLD_NAME_VIEW_REASON;
use sqlbuild_analysis::janitor_planning::main::build_archive_name::build_archive_name;
use sqlbuild_analysis::janitor_planning::main::query_diff_artifact_name::query_diff_artifact_name;
use sqlbuild_analysis::janitor_planning::main::query_diff_artifact_parts::query_diff_artifact_parts;
use sqlbuild_analysis::review_command::constants::LEFTOVER_REVIEW_SCHEMA_AGE_DAYS;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::janitor::janitor_classification::Candidate;
use crate::bindings::_helpers::janitor::janitor_classification::Policy;
use crate::bindings::_helpers::janitor::janitor_classification::Protection;
use crate::bindings::_helpers::janitor::janitor_classification::SchemaClassification;
use crate::bindings::_helpers::janitor::janitor_classification::classify_target_schemas;
use crate::bindings::_helpers::janitor::janitor_classification::plan_archives;
use crate::bindings::_helpers::janitor::janitor_facts::Warehouse;
use crate::bindings::_helpers::janitor::janitor_facts::WarehouseFacts;
use crate::bindings::_helpers::janitor::janitor_facts::collect_target_schemas;
use crate::bindings::_helpers::janitor::janitor_facts::direct_state_prune_candidates;
use crate::bindings::_helpers::janitor::janitor_facts::gather_warehouse_facts;
use crate::bindings::_helpers::janitor::janitor_facts::query_diff_artifact_candidates;
use crate::bindings::_helpers::janitor::janitor_keys::Folded;
use crate::bindings::_helpers::janitor::janitor_keys::Key;
use crate::bindings::_helpers::janitor::janitor_keys::SchemaKey;
use crate::bindings::_helpers::janitor::janitor_keys::archive_timestamp;
use crate::bindings::_helpers::janitor::janitor_keys::days;
use crate::bindings::_helpers::janitor::janitor_keys::dedupe_schema_keys;
use crate::bindings::_helpers::janitor::janitor_keys::integer;
use crate::bindings::_helpers::janitor::janitor_keys::now_utc;
use crate::bindings::_helpers::janitor::janitor_keys::record;
use crate::bindings::_helpers::janitor::janitor_keys::sorted_schemas;
use crate::bindings::_helpers::janitor::janitor_keys::timestamp_fields;
use crate::bindings::_helpers::janitor::janitor_keys::tuple;
use crate::bindings::_helpers::janitor::janitor_keys::union_exact;
use crate::bindings::_helpers::janitor::janitor_old_names::OldNamePlanning;
use crate::bindings::_helpers::janitor::janitor_old_names::pending_migration_origins;
use crate::bindings::_helpers::janitor::janitor_old_names::plan_old_name_views;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_project::native_host::main::target_location::target_location;
use sqlbuild_python_project::relation_naming::main::shared_error::shared_error;

const LIFECYCLE: ExecutorHost = ExecutorHost::OperationLifecycle;

/// Settings of one janitor plan.
pub(crate) struct JanitorSettings {
    pub(crate) retention_days: i64,
    pub(crate) delete_tracked_only: bool,
    pub(crate) exclude_patterns: Vec<String>,
    pub(crate) direct_mode: bool,
    pub(crate) state_history_versions: i64,
    pub(crate) archive_retention_days: i64,
    pub(crate) early_old_name_view_drops: Vec<String>,
}

/// Relations scanned or protected beyond the project's target schemas.
pub(crate) struct RelationScope {
    pub(crate) scan_keys: Vec<Key>,
    pub(crate) protected_keys: Vec<Key>,
    pub(crate) reasons: Vec<(Key, String)>,
}

fn keys_of(items: &Bound<'_, PyAny>) -> PyResult<Vec<Key>> {
    items.try_iter()?.map(|item| Key::of(&item?)).collect()
}

fn bool_value(py: Python<'_>, value: bool) -> Bound<'_, PyAny> {
    PyBool::new(py, value).to_owned().into_any()
}

fn int_field(py: Python<'_>, value: usize) -> PyResult<Bound<'_, PyAny>> {
    integer(py, i64::try_from(value).unwrap_or(i64::MAX))
}

/// Mark an operation lifecycle completed with its item count.
pub(crate) fn completed(lifecycle: &Bound<'_, PyAny>, item_count: usize) -> PyResult<()> {
    let py: Python<'_> = lifecycle.py();
    let metadata: Bound<'_, PyDict> = PyDict::new(py);
    metadata.set_item("item_count", item_count)?;
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("metadata", metadata)?;
    lifecycle.call_method("completed", (), Some(&options))?;
    Ok(())
}

fn protection(scope: &RelationScope) -> Protection {
    Protection {
        keys: scope.protected_keys.iter().map(Key::folded).collect(),
        reasons: scope
            .reasons
            .iter()
            .map(|(key, reason)| (key.folded(), reason.clone()))
            .collect(),
    }
}

fn protect(
    protection: Protection,
    pairs: impl IntoIterator<Item = (Folded, String)>,
) -> Protection {
    let Protection {
        mut keys,
        mut reasons,
    } = protection;
    for (key, reason) in pairs {
        keys.insert(key.clone());
        reasons.insert(key, reason);
    }
    Protection { keys, reasons }
}

fn scan_schemas(managed: &[SchemaKey], scope: &RelationScope) -> Vec<SchemaKey> {
    let mut seen: HashSet<Folded> = HashSet::new();
    let mut scoped_keys: Vec<SchemaKey> = Vec::new();
    for key in scope.protected_keys.iter().chain(&scope.scan_keys) {
        if seen.insert(key.folded()) {
            scoped_keys.push((key.database.clone(), key.schema.clone()));
        }
    }
    let scoped: Vec<SchemaKey> = union_exact(Vec::new(), scoped_keys);
    dedupe_schema_keys(
        sorted_schemas(managed)
            .into_iter()
            .chain(sorted_schemas(&scoped)),
    )
}

fn candidate_records<'py>(candidates: &[Candidate<'py>]) -> PyResult<Vec<Bound<'py, PyAny>>> {
    candidates.iter().map(Candidate::to_python).collect()
}

/// Review schemas whose markers the active target schema holds and that outlived any review run.
fn review_schema_candidates<'py>(
    project: &Bound<'py, PyAny>,
    warehouse: &Warehouse<'py>,
    now: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = warehouse.py();
    let Some(schema) = target_location(project, "effective_target_schema")? else {
        return Ok(Vec::new());
    };
    let max_age: Bound<'py, PyAny> = days(py, LEFTOVER_REVIEW_SCHEMA_AGE_DAYS)?;
    let leftovers: Bound<'py, PyAny> = call_with(
        &host_entry(py, ExecutorHost::LeftoverReviewSchemas)?,
        &[
            ("adapter", warehouse.adapter.clone()),
            ("connection", warehouse.connection.clone()),
            (
                "database",
                target_location(project, "effective_target_database")?.into_bound_py_any(py)?,
            ),
            ("schema", schema.into_bound_py_any(py)?),
            ("now", now.clone()),
            ("max_age", max_age),
        ],
    )?;
    leftovers.try_iter()?.collect()
}

/// The plan for a project whose target schemas list nothing beyond query-diff artifacts.
fn artifact_only_plan<'py>(
    (project, warehouse): (&Bound<'py, PyAny>, &Warehouse<'py>),
    settings: &JanitorSettings,
    (artifacts, artifact_skips, scanned): (Vec<Bound<'py, PyAny>>, Vec<Bound<'py, PyAny>>, usize),
    (review_schemas, now): (Vec<Bound<'py, PyAny>>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = warehouse.py();
    record(
        py,
        ExecutorHost::JanitorPlan,
        &[
            (
                "target_name",
                native_field(project, "effective_target_name")?,
            ),
            ("retention_days", integer(py, settings.retention_days)?),
            ("direct_mode", bool_value(py, settings.direct_mode)),
            (
                "archive_retention_days",
                integer(py, settings.archive_retention_days)?,
            ),
            ("query_diff_artifact_candidates", tuple(py, artifacts)?),
            ("direct_state_prune_candidates", tuple(py, [])?),
            ("review_schema_candidates", tuple(py, review_schemas)?),
            ("skipped_relations", tuple(py, artifact_skips)?),
            ("scanned_schema_count", int_field(py, scanned)?),
            (
                "age_metadata_supported",
                bool_value(py, warehouse.age_metadata_support()?),
            ),
            ("planned_at", now.clone()),
        ],
    )
}

/// Build the desired-versus-warehouse cleanup plan for the project's target schemas.
pub(crate) fn build_plan<'py>(
    (project, warehouse): (&Bound<'py, PyAny>, &Warehouse<'py>),
    settings: &JanitorSettings,
    scope: &RelationScope,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = warehouse.py();
    let managed: Vec<SchemaKey> = collect_target_schemas(project)?;
    let target_schemas: Vec<SchemaKey> = scan_schemas(&managed, scope);
    let mut artifact_schemas: Vec<(Option<String>, String)> = Vec::new();
    let mut push_artifact_schema = |database: Option<String>, schema: String| {
        if !artifact_schemas.contains(&(database.clone(), schema.clone())) {
            artifact_schemas.push((database, schema));
        }
    };
    for (database, schema) in &target_schemas {
        if let Some(schema) = schema {
            push_artifact_schema(database.clone(), schema.clone());
        }
    }
    if let Some(schema) = target_location(project, "effective_target_schema")? {
        push_artifact_schema(
            target_location(project, "effective_target_database")?,
            schema,
        );
    }
    let now: Bound<'py, PyAny> = now_utc(py)?;
    let (artifacts, artifact_skips) =
        query_diff_artifact_candidates(warehouse, &artifact_schemas, &now)?;
    let review_schemas: Vec<Bound<'py, PyAny>> =
        review_schema_candidates(project, warehouse, &now)?;
    if target_schemas.is_empty() {
        let scanned: usize = artifact_schemas.len();
        return artifact_only_plan(
            (project, warehouse),
            settings,
            (artifacts, artifact_skips, scanned),
            (review_schemas, &now),
        );
    }
    let lifecycle: Bound<'py, PyAny> = host_entry(py, LIFECYCLE)?;
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("operation_kind", "janitor")?;
    options.set_item("operation_name", "janitor_warehouse_inspection")?;
    let (facts, state_prunes): (WarehouseFacts<'py>, Vec<Bound<'py, PyAny>>) =
        within(&lifecycle.call((), Some(&options))?, |inspection| {
            let facts: WarehouseFacts<'py> = gather_warehouse_facts(
                (project, warehouse),
                &target_schemas,
                settings.delete_tracked_only,
            )?;
            let prunes: Vec<Bound<'py, PyAny>> = direct_state_prune_candidates(
                warehouse,
                &target_schemas,
                settings.state_history_versions,
            )?;
            completed(inspection, target_schemas.len())?;
            Ok((facts, prunes))
        })?;
    let old_names: OldNamePlanning<'py> = plan_old_name_views(
        (project, warehouse),
        (&managed, &facts.relations),
        (&settings.early_old_name_view_drops, &now),
    )?;
    let protection: Protection = protect(
        protection(scope),
        old_names
            .keys
            .iter()
            .map(|key| (key.clone(), OLD_NAME_VIEW_REASON.to_owned())),
    );
    let pending: Vec<(Folded, String)> =
        pending_migration_origins(project, (&facts.relations, &managed))?
            .into_iter()
            .filter(|(key, _)| !old_names.keys.contains(key))
            .collect();
    let protection: Protection = protect(protection, pending);
    let age_supported: bool = warehouse.age_metadata_support()?;
    let policy = Policy {
        exclude_patterns: &settings.exclude_patterns,
        delete_tracked_only: settings.delete_tracked_only,
        retention_days: settings.retention_days,
        direct_mode: settings.direct_mode,
    };
    let schemas: SchemaClassification<'py> = classify_target_schemas(
        (warehouse, &facts),
        (&target_schemas, &managed),
        (&protection, &policy),
        &now,
    )?;
    let archives = plan_archives(
        (warehouse, &facts),
        (&schemas, &managed),
        (&policy, settings.archive_retention_days),
        &now,
    )?;
    let mut skipped: Vec<Bound<'py, PyAny>> = Vec::new();
    for relation in &schemas.skipped {
        if query_diff_artifact_parts(&relation.key.name).is_none()
            && !old_names.keys.contains(&relation.key.folded())
        {
            skipped.push(relation.record.clone());
        }
    }
    skipped.extend(archives.skipped);
    skipped.extend(artifact_skips);
    let scanned: Vec<SchemaKey> = union_exact(
        target_schemas.clone(),
        artifact_schemas
            .into_iter()
            .map(|(database, schema)| (database, Some(schema))),
    );
    record(
        py,
        ExecutorHost::JanitorPlan,
        &[
            (
                "target_name",
                native_field(project, "effective_target_name")?,
            ),
            ("retention_days", integer(py, settings.retention_days)?),
            ("direct_mode", bool_value(py, settings.direct_mode)),
            (
                "archive_retention_days",
                integer(py, settings.archive_retention_days)?,
            ),
            (
                "candidates",
                tuple(py, candidate_records(&schemas.candidates)?)?,
            ),
            (
                "archive_candidates",
                tuple(py, archives.archive_candidates)?,
            ),
            (
                "archive_deletion_candidates",
                tuple(py, archives.deletions)?,
            ),
            ("retained_archives", tuple(py, archives.retained)?),
            ("query_diff_artifact_candidates", tuple(py, artifacts)?),
            ("direct_state_prune_candidates", tuple(py, state_prunes)?),
            ("review_schema_candidates", tuple(py, review_schemas)?),
            ("skipped_relations", tuple(py, skipped)?),
            ("skipped_schemas", tuple(py, schemas.skipped_schemas)?),
            ("blocked_schemas", tuple(py, archives.blocked_schemas)?),
            ("old_name_views", old_names.planning),
            ("scanned_schema_count", int_field(py, scanned.len())?),
            ("age_metadata_supported", bool_value(py, age_supported)),
            ("planned_at", now),
        ],
    )
}

fn reasons_of(value: &Bound<'_, PyAny>) -> PyResult<Vec<(Key, String)>> {
    if value.is_none() {
        return Ok(Vec::new());
    }
    let mut reasons: Vec<(Key, String)> = Vec::new();
    for item in mapping_items(value)?.try_iter()? {
        let (key, reason): (Bound<'_, PyAny>, String) = item?.extract()?;
        reasons.push((Key::of(&key)?, reason));
    }
    Ok(reasons)
}

fn optional_keys(value: &Bound<'_, PyAny>) -> PyResult<Vec<Key>> {
    if value.is_none() {
        return Ok(Vec::new());
    }
    keys_of(value)
}

fn value_or<'py, T: pyo3::prelude::FromPyObjectOwned<'py>>(
    arguments: &KeywordArguments<'py>,
    name: &str,
    default: T,
) -> PyResult<T> {
    if arguments.passed(name)? {
        return arguments.value(name)?.extract().map_err(Into::into);
    }
    Ok(default)
}

/// Build the janitor cleanup plan; adapter reads stay adapter callbacks in the executor's order.
#[pyfunction]
#[pyo3(name = "build_janitor_plan", signature = (**options))]
pub(crate) fn build_janitor_plan_binding<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_janitor_plan",
        options,
        (
            &["project", "adapter", "connection", "retention_days"],
            &[
                "delete_tracked_only",
                "exclude_patterns",
                "direct_mode",
                "state_history_versions",
                "archive_retention_days",
                "early_old_name_view_drops",
                "scan_relation_keys",
                "protected_relation_keys",
                "protected_relation_reasons",
            ],
        ),
    )?;
    let scope = RelationScope {
        scan_keys: optional_keys(&arguments.value("scan_relation_keys")?)?,
        protected_keys: optional_keys(&arguments.value("protected_relation_keys")?)?,
        reasons: reasons_of(&arguments.value("protected_relation_reasons")?)?,
    };
    let settings = JanitorSettings {
        retention_days: arguments.value("retention_days")?.extract()?,
        delete_tracked_only: value_or(&arguments, "delete_tracked_only", true)?,
        exclude_patterns: value_or(&arguments, "exclude_patterns", Vec::new())?,
        direct_mode: value_or(&arguments, "direct_mode", true)?,
        state_history_versions: value_or(&arguments, "state_history_versions", 20)?,
        archive_retention_days: value_or(&arguments, "archive_retention_days", 14)?,
        early_old_name_view_drops: value_or(&arguments, "early_old_name_view_drops", Vec::new())?,
    };
    let warehouse = Warehouse {
        adapter: arguments.value("adapter")?,
        connection: arguments.value("connection")?,
    };
    build_plan(
        (&arguments.value("project")?, &warehouse),
        &settings,
        &scope,
    )
}

/// A strict archive name the janitor expires by its embedded UTC timestamp.
#[pyfunction]
#[pyo3(signature = (*, original_name, archived_at, identifier_limit, kind = None))]
pub(crate) fn build_janitor_archive_name(
    original_name: &str,
    archived_at: &Bound<'_, PyAny>,
    identifier_limit: i64,
    kind: Option<&str>,
) -> PyResult<String> {
    build_archive_name(
        (
            original_name,
            timestamp_fields(&archive_timestamp(archived_at)?)?,
        ),
        identifier_limit,
        kind,
    )
    .map_err(|message| shared_error(archived_at.py(), message))
}

/// The reserved relation name of one run's query-diff artifact side.
#[pyfunction]
#[pyo3(signature = (*, run_id, side))]
pub(crate) fn build_query_diff_artifact_name(run_id: &str, side: &str) -> String {
    query_diff_artifact_name(run_id, side)
}

/// The run id and folded side of a strict query-diff artifact name, or `None`.
#[pyfunction]
#[pyo3(signature = (name, /))]
pub(crate) fn parse_query_diff_artifact_name(name: &str) -> Option<(String, String)> {
    query_diff_artifact_parts(name)
}
