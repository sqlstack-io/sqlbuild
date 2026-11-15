//! Delete candidates, skipped relations, blocked schemas and direct-mode archives per schema.

use std::collections::{BTreeSet, HashMap, HashSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_analysis::janitor_planning::constants::{
    AGE_UNAVAILABLE_REASON, AGE_UNSUPPORTED_REASON, BUILT_IN_EXCLUDE_PATTERNS,
    MALFORMED_ARCHIVE_REASON, PROTECTED_REASON, QUERY_DIFF_ARTIFACT_REASON, UNTRACKED_REASON,
};
use sqlbuild_analysis::janitor_planning::main::build_archive_name::build_archive_name;
use sqlbuild_analysis::janitor_planning::main::case_colliding_names::case_colliding_names;
use sqlbuild_analysis::janitor_planning::main::case_collision_reason::case_collision_reason;
use sqlbuild_analysis::janitor_planning::main::is_archive_lookalike_name::is_archive_lookalike_name;
use sqlbuild_analysis::janitor_planning::main::matching_exclude_pattern::matching_exclude_pattern;
use sqlbuild_analysis::janitor_planning::main::parse_archive_name::parse_archive_name;
use sqlbuild_analysis::janitor_planning::main::query_diff_artifact_parts::query_diff_artifact_parts;
use sqlbuild_analysis::janitor_planning::main::unaddressable_relation_reason::unaddressable_relation_reason;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::janitor::janitor_facts::{
    Warehouse, WarehouseFacts, skipped_relation,
};
use crate::bindings::_helpers::janitor::janitor_keys::{
    Folded, Key, SchemaKey, archive_timestamp, days, folded_schema, optional, record,
    relation_age_timestamp, sorted_schemas, text, timestamp_fields, tuple, utc_datetime,
};
use crate::bindings::_helpers::planner_scenario::artifacts::is_scenario_artifact_physical_name;
use crate::bindings::_helpers::relation_naming::fitting::shared_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Protection rules for one plan.
pub(crate) struct Protection {
    pub(crate) keys: HashSet<Folded>,
    pub(crate) reasons: HashMap<Folded, String>,
}

/// Relation-level settings for one plan.
pub(crate) struct Policy<'a> {
    pub(crate) exclude_patterns: &'a [String],
    pub(crate) delete_tracked_only: bool,
    pub(crate) retention_days: i64,
    pub(crate) direct_mode: bool,
}

/// One stale relation eligible for deletion or archiving.
pub(crate) struct Candidate<'py> {
    pub(crate) key: Key,
    pub(crate) relation: Bound<'py, PyAny>,
    pub(crate) age_timestamp: Option<Bound<'py, PyAny>>,
}

impl<'py> Candidate<'py> {
    pub(crate) fn to_python(&self) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = self.relation.py();
        record(
            py,
            ExecutorHost::JanitorDeleteCandidate,
            &[
                ("key", self.key.to_python(py)?),
                ("relation", self.relation.clone()),
                (
                    "age_timestamp",
                    age_or_none(py, self.age_timestamp.as_ref()),
                ),
            ],
        )
    }
}

/// A skipped relation and its key.
pub(crate) struct Skipped<'py> {
    pub(crate) key: Key,
    pub(crate) record: Bound<'py, PyAny>,
}

fn skipped<'py>(key: Key, relation: &Bound<'py, PyAny>, reason: &str) -> PyResult<Skipped<'py>> {
    Ok(Skipped {
        record: skipped_relation(&key, relation, reason)?,
        key,
    })
}

/// A source-overlap schema blocked in direct mode.
pub(crate) struct Blocked<'py> {
    pub(crate) schema: SchemaKey,
    pub(crate) source_names: Vec<String>,
    pub(crate) suppressed: Vec<Candidate<'py>>,
}

/// Relation decisions across target schemas.
pub(crate) struct SchemaClassification<'py> {
    pub(crate) candidates: Vec<Candidate<'py>>,
    pub(crate) skipped: Vec<Skipped<'py>>,
    pub(crate) skipped_schemas: Vec<Bound<'py, PyAny>>,
    pub(crate) blocked: Vec<Blocked<'py>>,
}

fn age_or_none<'py>(py: Python<'py>, value: Option<&Bound<'py, PyAny>>) -> Bound<'py, PyAny> {
    value.map_or_else(|| py.None().into_bound(py), Clone::clone)
}

fn effective_patterns(exclude_patterns: &[String]) -> Vec<String> {
    BUILT_IN_EXCLUDE_PATTERNS
        .iter()
        .map(|pattern| (*pattern).to_owned())
        .chain(exclude_patterns.iter().cloned())
        .collect()
}

fn exclude_reason(py: Python<'_>, key: &Key, patterns: &[String]) -> PyResult<Option<String>> {
    let Some(pattern) = matching_exclude_pattern((&key.name, &key.display_name()), patterns) else {
        return Ok(None);
    };
    let quoted: String = PyString::new(py, pattern).repr()?.extract()?;
    Ok(Some(format!("relation matches exclude pattern {quoted}")))
}

fn relation_skip_reason(
    py: Python<'_>,
    (key, facts): (&Key, &WarehouseFacts<'_>),
    (protection, policy): (&Protection, &Policy<'_>),
    patterns: &[String],
) -> PyResult<Option<String>> {
    if query_diff_artifact_parts(&key.name).is_some() {
        return Ok(Some(QUERY_DIFF_ARTIFACT_REASON.to_owned()));
    }
    let folded: Folded = key.folded();
    if protection.keys.contains(&folded) {
        return Ok(Some(
            protection
                .reasons
                .get(&folded)
                .cloned()
                .unwrap_or_else(|| PROTECTED_REASON.to_owned()),
        ));
    }
    if let Some(reason) = exclude_reason(py, key, patterns)? {
        return Ok(Some(reason));
    }
    if policy.delete_tracked_only
        && !facts.tracked.contains(&folded)
        && !is_scenario_artifact_physical_name(&key.name)
    {
        return Ok(Some(UNTRACKED_REASON.to_owned()));
    }
    Ok(None)
}

fn retention_skip_reason(
    (age_timestamp, age_supported): (Option<&Bound<'_, PyAny>>, bool),
    retention_days: i64,
    now: &Bound<'_, PyAny>,
) -> PyResult<Option<String>> {
    if !age_supported {
        return Ok(Some(AGE_UNSUPPORTED_REASON.to_owned()));
    }
    let Some(age_timestamp) = age_timestamp else {
        return Ok(Some(AGE_UNAVAILABLE_REASON.to_owned()));
    };
    if age_timestamp.gt(now.sub(days(now.py(), retention_days)?)?)? {
        return Ok(Some(format!(
            "relation is newer than {retention_days} days"
        )));
    }
    Ok(None)
}

/// Relations with age metadata filled when the adapter supports it.
fn read_ages<'py>(
    warehouse: &Warehouse<'py>,
    relations: Vec<Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    if relations.is_empty() || !warehouse.age_metadata_support()? {
        return Ok(relations);
    }
    let py: Python<'py> = warehouse.py();
    warehouse
        .call(
            "with_relation_age_metadata",
            &[("relations", tuple(py, relations)?)],
        )?
        .try_iter()?
        .collect()
}

/// One schema's relations split into delete candidates and skipped relations.
fn classify_relations<'py>(
    (warehouse, facts): (&Warehouse<'py>, &WarehouseFacts<'py>),
    schema_relations: &[Bound<'py, PyAny>],
    (protection, policy): (&Protection, &Policy<'_>),
    now: &Bound<'py, PyAny>,
) -> PyResult<(Vec<Candidate<'py>>, Vec<Skipped<'py>>)> {
    let py: Python<'py> = warehouse.py();
    let patterns: Vec<String> = effective_patterns(policy.exclude_patterns);
    let keys: Vec<Key> = schema_relations
        .iter()
        .map(Key::of)
        .collect::<PyResult<_>>()?;
    let colliding: HashSet<String> = case_colliding_names(keys.iter().map(|key| key.name.as_str()));
    let mut skipped_relations: Vec<Skipped<'py>> = Vec::new();
    let mut eligible: Vec<Bound<'py, PyAny>> = Vec::new();
    for (key, relation) in keys.into_iter().zip(schema_relations) {
        if facts.desired.contains(&key.folded()) {
            continue;
        }
        if policy.direct_mode && is_archive_lookalike_name(&key.name) {
            continue;
        }
        let mut reason: Option<String> =
            relation_skip_reason(py, (&key, facts), (protection, policy), &patterns)?;
        if reason.is_none() {
            reason = if policy.direct_mode {
                unaddressable_relation_reason(&key.name, &colliding)
            } else {
                case_collision_reason(&key.name, &colliding)
            }
            .map(str::to_owned);
        }
        match reason {
            Some(reason) => skipped_relations.push(skipped(key, relation, &reason)?),
            None => eligible.push(relation.clone()),
        }
    }
    let age_supported: bool = warehouse.age_metadata_support()?;
    let aged: Vec<Bound<'py, PyAny>> = if policy.retention_days > 0 {
        read_ages(warehouse, eligible)?
    } else {
        eligible
    };
    let mut candidates: Vec<Candidate<'py>> = Vec::new();
    for relation in aged {
        let key: Key = Key::of(&relation)?;
        let age_timestamp: Option<Bound<'py, PyAny>> = relation_age_timestamp(&relation)?;
        let retention_reason: Option<String> = if policy.retention_days > 0 {
            retention_skip_reason(
                (age_timestamp.as_ref(), age_supported),
                policy.retention_days,
                now,
            )?
        } else {
            None
        };
        if let Some(reason) = retention_reason {
            skipped_relations.push(skipped(key, &relation, &reason)?);
            continue;
        }
        candidates.push(Candidate {
            key,
            relation,
            age_timestamp,
        });
    }
    Ok((candidates, skipped_relations))
}

/// All target schemas classified, keeping relations in schemas with configured sources.
pub(crate) fn classify_target_schemas<'py>(
    (warehouse, facts): (&Warehouse<'py>, &WarehouseFacts<'py>),
    (target_schemas, managed): (&[SchemaKey], &[SchemaKey]),
    (protection, policy): (&Protection, &Policy<'_>),
    now: &Bound<'py, PyAny>,
) -> PyResult<SchemaClassification<'py>> {
    let py: Python<'py> = warehouse.py();
    let mut result = SchemaClassification {
        candidates: Vec::new(),
        skipped: Vec::new(),
        skipped_schemas: Vec::new(),
        blocked: Vec::new(),
    };
    for schema_key in sorted_schemas(target_schemas) {
        let schema_relations: &[Bound<'py, PyAny>] = facts.schema_relations(&schema_key);
        let source_names: BTreeSet<String> = facts.source_names(&schema_key);
        let folded: SchemaKey = folded_schema(&schema_key);
        let blocking: Vec<String> = if managed.iter().any(|key| folded_schema(key) == folded) {
            source_names.iter().cloned().collect()
        } else {
            Vec::new()
        };
        if !source_names.is_empty() || !blocking.is_empty() {
            if policy.direct_mode && !blocking.is_empty() {
                let (suppressed, _) = classify_relations(
                    (warehouse, facts),
                    schema_relations,
                    (protection, policy),
                    now,
                )?;
                result.blocked.push(Blocked {
                    schema: schema_key,
                    source_names: blocking,
                    suppressed,
                });
                continue;
            }
            result.skipped_schemas.push(record(
                py,
                ExecutorHost::JanitorSkippedSchema,
                &[
                    ("database", optional(py, schema_key.0.as_deref())?),
                    ("schema", optional(py, schema_key.1.as_deref())?),
                    (
                        "source_names",
                        tuple(py, source_names.iter().map(|name| text(py, name)))?,
                    ),
                    (
                        "skipped_relations",
                        tuple(py, schema_relations.iter().cloned())?,
                    ),
                ],
            )?);
            continue;
        }
        let (candidates, skipped_relations) = classify_relations(
            (warehouse, facts),
            schema_relations,
            (protection, policy),
            now,
        )?;
        result.candidates.extend(candidates);
        result.skipped.extend(skipped_relations);
    }
    Ok(result)
}

/// Direct-mode archive renames and name-based archive expiry.
pub(crate) struct ArchivePlanning<'py> {
    pub(crate) archive_candidates: Vec<Bound<'py, PyAny>>,
    pub(crate) deletions: Vec<Bound<'py, PyAny>>,
    pub(crate) retained: Vec<Bound<'py, PyAny>>,
    pub(crate) skipped: Vec<Bound<'py, PyAny>>,
    pub(crate) blocked_schemas: Vec<Bound<'py, PyAny>>,
}

fn archived_relation<'py>(
    (key, relation_type): (&Key, Bound<'py, PyAny>),
    (archived_at, expires_at): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    original_key: Option<&Key>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = archived_at.py();
    let mut pairs: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("key", key.to_python(py)?),
        ("relation_type", relation_type),
        ("archived_at", archived_at.clone()),
        ("expires_at", expires_at.clone()),
    ];
    if let Some(original_key) = original_key {
        pairs.push(("original_key", original_key.to_python(py)?));
    }
    record(py, ExecutorHost::JanitorArchivedRelation, &pairs)
}

fn blocked_record<'py>(
    py: Python<'py>,
    schema: &Blocked<'py>,
    suppressed_archives: &HashMap<SchemaKey, Vec<Bound<'py, PyAny>>>,
) -> PyResult<Bound<'py, PyAny>> {
    let source_names: Vec<Bound<'py, PyAny>> = schema
        .source_names
        .iter()
        .map(|name| text(py, name))
        .collect();
    let suppressed: Vec<Bound<'py, PyAny>> = schema
        .suppressed
        .iter()
        .map(Candidate::to_python)
        .collect::<PyResult<_>>()?;
    let archives: Vec<Bound<'py, PyAny>> = suppressed_archives
        .get(&schema.schema)
        .cloned()
        .unwrap_or_default();
    record(
        py,
        ExecutorHost::JanitorBlockedSchema,
        &[
            ("database", optional(py, schema.schema.0.as_deref())?),
            ("schema", optional(py, schema.schema.1.as_deref())?),
            ("source_names", tuple(py, source_names)?),
            ("suppressed_candidates", tuple(py, suppressed)?),
            ("suppressed_archive_deletions", tuple(py, archives)?),
        ],
    )
}

fn blocked_records<'py>(
    py: Python<'py>,
    blocked: &[Blocked<'py>],
    suppressed_archives: &HashMap<SchemaKey, Vec<Bound<'py, PyAny>>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    blocked
        .iter()
        .map(|schema| blocked_record(py, schema, suppressed_archives))
        .collect()
}

fn archive_candidate<'py>(
    candidate: &Candidate<'py>,
    (archived_at, expires_at): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    identifier_limit: i64,
) -> PyResult<(Key, Bound<'py, PyAny>)> {
    let py: Python<'py> = archived_at.py();
    let archive_key = Key {
        database: candidate.key.database.clone(),
        schema: candidate.key.schema.clone(),
        name: build_archive_name(
            (&candidate.key.name, timestamp_fields(archived_at)?),
            identifier_limit,
            None,
        )
        .map_err(|message| shared_error(py, message))?,
    };
    let record = record(
        py,
        ExecutorHost::JanitorArchiveCandidate,
        &[
            ("key", candidate.key.to_python(py)?),
            ("relation", candidate.relation.clone()),
            (
                "age_timestamp",
                age_or_none(py, candidate.age_timestamp.as_ref()),
            ),
            ("archive_key", archive_key.to_python(py)?),
            ("archived_at", archived_at.clone()),
            ("expires_at", expires_at.clone()),
        ],
    )?;
    Ok((archive_key, record))
}

/// Archive renames and expiry of existing archives; non-direct mode keeps only blocked schemas.
pub(crate) fn plan_archives<'py>(
    (warehouse, facts): (&Warehouse<'py>, &WarehouseFacts<'py>),
    (schemas, managed): (&SchemaClassification<'py>, &[SchemaKey]),
    (policy, archive_retention_days): (&Policy<'_>, i64),
    now: &Bound<'py, PyAny>,
) -> PyResult<ArchivePlanning<'py>> {
    let py: Python<'py> = warehouse.py();
    if !policy.direct_mode {
        return Ok(ArchivePlanning {
            archive_candidates: Vec::new(),
            deletions: Vec::new(),
            retained: Vec::new(),
            skipped: Vec::new(),
            blocked_schemas: blocked_records(py, &schemas.blocked, &HashMap::new())?,
        });
    }
    let identifier_limit: i64 = warehouse
        .adapter
        .call_method0("maximum_identifier_length")?
        .extract()?;
    let archived_at: Bound<'py, PyAny> = archive_timestamp(now)?;
    let retention: Bound<'py, PyAny> = days(py, archive_retention_days)?;
    let expires_at: Bound<'py, PyAny> = archived_at.add(&retention)?;
    let mut archive_candidates: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut same_run: Vec<Bound<'py, PyAny>> = Vec::new();
    let same_run_expiry: bool = expires_at.le(&archived_at)?;
    for candidate in &schemas.candidates {
        let (archive_key, record) =
            archive_candidate(candidate, (&archived_at, &expires_at), identifier_limit)?;
        archive_candidates.push(record);
        if same_run_expiry {
            same_run.push(archived_relation(
                (
                    &archive_key,
                    native_field(&candidate.relation, "relation_type")?,
                ),
                (&archived_at, &expires_at),
                Some(&candidate.key),
            )?);
        }
    }
    let blocked_keys: Vec<&SchemaKey> = schemas
        .blocked
        .iter()
        .map(|blocked| &blocked.schema)
        .collect();
    let patterns: Vec<String> = effective_patterns(policy.exclude_patterns);
    let mut deletions: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut retained: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut skipped_relations: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut suppressed: HashMap<SchemaKey, Vec<Bound<'py, PyAny>>> = HashMap::new();
    for schema_key in sorted_schemas(managed) {
        let schema_relations: &[Bound<'py, PyAny>] = facts.schema_relations(&schema_key);
        let keys: Vec<Key> = schema_relations
            .iter()
            .map(Key::of)
            .collect::<PyResult<_>>()?;
        let colliding: HashSet<String> =
            case_colliding_names(keys.iter().map(|key| key.name.as_str()));
        for (key, relation) in keys.into_iter().zip(schema_relations) {
            if facts.desired.contains(&key.folded()) || !is_archive_lookalike_name(&key.name) {
                continue;
            }
            let Some(parsed) = parse_archive_name(&key.name) else {
                skipped_relations.push(skipped_relation(&key, relation, MALFORMED_ARCHIVE_REASON)?);
                continue;
            };
            let reason: Option<String> = match unaddressable_relation_reason(&key.name, &colliding)
            {
                Some(reason) => Some(reason.to_owned()),
                None => exclude_reason(py, &key, &patterns)?,
            };
            if let Some(reason) = reason {
                skipped_relations.push(skipped_relation(&key, relation, &reason)?);
                continue;
            }
            let parsed_at: Bound<'py, PyAny> = utc_datetime(py, parsed.archived_at)?;
            let parsed_expiry: Bound<'py, PyAny> = parsed_at.add(&retention)?;
            let archived: Bound<'py, PyAny> = archived_relation(
                (&key, native_field(relation, "relation_type")?),
                (&parsed_at, &parsed_expiry),
                None,
            )?;
            if parsed_expiry.gt(now)? {
                retained.push(archived);
            } else if blocked_keys.contains(&&schema_key) {
                suppressed
                    .entry(schema_key.clone())
                    .or_default()
                    .push(archived);
            } else {
                deletions.push(archived);
            }
        }
    }
    deletions.extend(same_run);
    Ok(ArchivePlanning {
        archive_candidates,
        deletions,
        retained,
        skipped: skipped_relations,
        blocked_schemas: blocked_records(py, &schemas.blocked, &suppressed)?,
    })
}
