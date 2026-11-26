//! Selected-scope buildability validation before change planning.

use crate::planner_phases::_helpers::inputs::contexts::class_field;
use crate::planner_phases::_helpers::inputs::runtime::runtime_field;
use crate::planner_phases::_helpers::inputs::runtime::warehouse_field;
use crate::planner_phases::constants::{AUDIT_KIND, MISSING_UPSTREAM_CODE};
use crate::planner_phases::main::inputs::runtime_dialect::runtime_dialect;
use crate::planner_phases::main::inputs::runtime_reads::runtime_reads;
use crate::planner_strategy::main::audit_entries::audit_is_selected::audit_is_selected;
use crate::planner_warehouse::main::sources::check_selected_source_tables_exist::check_selected_source_tables_exist;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_analysis::planner_identity::main::missing_upstream_message::missing_upstream_message;
use sqlbuild_analysis::planner_identity::main::missing_upstreams::missing_upstreams;
use sqlbuild_analysis::planner_identity::models::{
    AvailableUpstreams, GraphKey, MissingUpstreamFixes,
};
use sqlbuild_python_base::records::main::construct::new_error_with_fields::new_error_with_fields;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_front::contracts::main::resolve_target_config::resolve_target_config;
use sqlbuild_python_front::contracts::main::resolve_target_name::resolve_target_name;
use sqlbuild_python_project::compiled_views::main::project::audits::audits;
use sqlbuild_python_project::compiled_views::main::project::seeds::seeds;
use sqlbuild_python_project::graph::main::nodes::native_key::native_key;
use sqlbuild_python_project::planner_identity::main::execution_origins::execution_origins;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;
use sqlbuild_python_project::planner_scope::models::PlannerScopeResolution;
use sqlbuild_python_project::planner_scope::models::ScopeData;
use sqlbuild_python_project::resource_views::main::audit_reads::attached_target_kind::attached_target_kind;
use sqlbuild_python_project::resource_views::main::audit_reads::attached_target_name::attached_target_name;
use sqlbuild_python_project::resource_views::main::audit_reads::name::name;
use sqlbuild_python_project::resource_views::main::audit_reads::scope_deps::scope_deps;
use sqlbuild_python_project::resource_views::main::seed_reads::external::external;
use sqlbuild_python_project::resource_views::main::seed_reads::key::key;
use std::collections::{BTreeMap, HashMap};

/// Refuse a selected scope whose reads don't exist; the end-of-build audits whose reads don't exist and that aren't selected by name are returned as not run, by name with what they miss.
pub(crate) fn check_selected_scope_buildability<'py>(
    runtime: &Bound<'py, PyAny>,
    scopes: &Bound<'py, PyAny>,
    warehouse: &Bound<'py, PyAny>,
    deferral: &Bound<'py, PyAny>,
) -> PyResult<BTreeMap<String, Vec<String>>> {
    let py = runtime.py();
    let project = runtime_field(runtime, |runtime| &runtime.project)?;
    let selected_scope = class_field(scopes, |s: &PlannerScopeResolution| &s.selected_scope)?;
    let scope = scope_data(&selected_scope)?;
    let mut satisfied_keys: std::collections::HashSet<GraphKey> = std::collections::HashSet::new();
    for seed in seeds(&project)?.try_iter()? {
        let seed = seed?;
        if external(&seed)?.is_truthy()? {
            satisfied_keys.insert(GraphKey::from(&native_key(&key(&seed)?)?));
        }
    }
    let snapshot = warehouse_field(warehouse, |warehouse| &warehouse.snapshot)?;
    let names = |value: Bound<'py, PyAny>| -> PyResult<std::collections::HashSet<String>> {
        value.try_iter()?.map(|value| value?.extract()).collect()
    };
    let available = AvailableUpstreams {
        satisfied_keys,
        existing_relations: names(native_field(&snapshot, "existing_relations")?)?,
        deferred_relations: deferred_model_names(deferral)?,
    };
    let selected: Vec<_> = scope.selected.iter().map(GraphKey::from).collect();
    let deps: std::collections::HashMap<GraphKey, Vec<GraphKey>> = scope
        .upstream
        .iter()
        .map(|(key, deps)| {
            (
                GraphKey::from(key),
                deps.iter().map(GraphKey::from).collect(),
            )
        })
        .collect();
    let mut missing = missing_upstreams(&selected, &deps, &available);
    let mut audit_origins: HashMap<(GraphKey, GraphKey), String> = HashMap::new();
    let mut not_run: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut built = available.clone();
    built.satisfied_keys.extend(selected.iter().cloned());
    for audit in end_audits(&project, &scope)? {
        let reads = HashMap::from([(audit.key.clone(), audit.reads)]);
        let misses = missing_upstreams(std::slice::from_ref(&audit.key), &reads, &built);
        if misses.is_empty() {
            continue;
        }
        if !audit.named {
            let names: Vec<String> = misses.iter().map(|miss| miss.key.name.clone()).collect();
            not_run.insert(audit.key.name.clone(), names);
            continue;
        }
        for miss in &misses {
            let label: String = match &audit.target {
                Some(target) => format!(
                    "audit '{}' on '{target}' reads '{}'",
                    audit.key.name, miss.key.name
                ),
                None => format!("audit '{}' reads '{}'", audit.key.name, miss.key.name),
            };
            audit_origins.insert((audit.key.clone(), miss.key.clone()), label);
        }
        missing.extend(misses);
    }
    if !missing.is_empty() {
        let mut origins: HashMap<(GraphKey, GraphKey), String> = execution_origins(&project)?
            .into_iter()
            .map(|((key, read), label)| ((GraphKey::from(&key), GraphKey::from(&read)), label))
            .collect();
        origins.extend(audit_origins);
        let message = missing_upstream_message(
            &missing,
            &origins,
            &missing_upstream_fixes(runtime, &project, deferred_to(deferral, &available)?)?,
        );
        let names = missing.iter().map(|entry| entry.key.name.as_str());
        return Err(new_error_with_fields(
            py,
            ErrorId::PlannerInput,
            (&message, Some(MISSING_UPSTREAM_CODE), None),
            &[("missing", PyTuple::new(py, names)?.into_any())],
        ));
    }
    check_selected_source_tables_exist(
        &project,
        (&runtime_dialect(runtime)?, &runtime_reads(runtime)?),
        &selected_scope,
        &warehouse_field(warehouse, |warehouse| &warehouse.inspection_relations)?,
    )?;
    Ok(not_run)
}

/// An audit that runs after the build, with what it reads and whether it was selected by name.
pub(crate) struct EndAudit {
    key: GraphKey,
    target: Option<String>,
    pub(crate) reads: Vec<GraphKey>,
    named: bool,
}

/// The selected audits that run after the build: singular audits, and named attached audits whose target isn't built; unnamed attached audits are skipped first.
pub(crate) fn end_audits(project: &Bound<'_, PyAny>, scope: &ScopeData) -> PyResult<Vec<EndAudit>> {
    let mut out: Vec<EndAudit> = Vec::new();
    for audit in audits(project)?.try_iter()? {
        let audit = audit?;
        let audit_name: String = name(&audit)?.extract()?;
        let named: bool = scope.audits.contains(&audit_name);
        let target: Option<String> = attached_target_name(&audit)?.extract()?;
        if let Some(target) = &target {
            if !named {
                continue;
            }
            let kind = attached_target_kind(&audit)?;
            let kind: String = match kind.getattr_opt("value")? {
                Some(value) => value.extract()?,
                None => kind.extract()?,
            };
            if scope.selected.contains(&(kind, target.clone())) {
                continue;
            }
        } else if !named && !audit_is_selected(&audit, (&scope.selected, &scope.audits))? {
            continue;
        }
        let mut reads: Vec<GraphKey> = Vec::new();
        for read in scope_deps(&audit)?.try_iter()? {
            reads.push(GraphKey::from(&native_key(&read?)?));
        }
        out.push(EndAudit {
            key: GraphKey {
                kind: AUDIT_KIND.to_owned(),
                name: audit_name,
            },
            target,
            reads,
            named,
        });
    }
    Ok(out)
}

/// The models and seeds whose deferred location, under an alias or a `migrate_from` origin, the deferred target lists.
fn deferred_model_names(
    deferral: &Bound<'_, PyAny>,
) -> PyResult<Option<std::collections::HashSet<String>>> {
    let relations = native_field(deferral, "deferred_relations")?;
    let locations = native_field(deferral, "deferred_locations")?;
    if relations.is_none() || locations.is_none() {
        return Ok(None);
    }
    let mut listed: Vec<RelationPlace> = Vec::new();
    for relation in relations.cast::<PyDict>()?.values() {
        listed.push(relation_place(&relation)?);
    }
    let mut names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (name, location) in locations.cast::<PyDict>()? {
        let place: RelationPlace = relation_place(&location)?;
        if listed.iter().any(|relation| same_place(relation, &place)) {
            names.insert(name.extract()?);
        }
    }
    Ok(Some(names))
}

/// A relation's lowercased `(database, schema, name)`.
type RelationPlace = (Option<String>, Option<String>, String);

/// Where a listed relation or a deferred location is, lowercased.
fn relation_place(relation: &Bound<'_, PyAny>) -> PyResult<RelationPlace> {
    let lower = |field: &str| -> PyResult<Option<String>> {
        Ok(native_field(relation, field)?
            .extract::<Option<String>>()?
            .map(|value| value.to_lowercase()))
    };
    Ok((
        lower("database")?,
        lower("schema")?,
        native_field(relation, "name")?
            .extract::<String>()?
            .to_lowercase(),
    ))
}

/// The same name, schema and database, where a part either side leaves to the connection's default matches any.
fn same_place(listed: &RelationPlace, location: &RelationPlace) -> bool {
    let part = |left: &Option<String>, right: &Option<String>| {
        left.is_none() || right.is_none() || left == right
    };
    listed.2 == location.2 && part(&listed.1, &location.1) && part(&listed.0, &location.0)
}

/// The target this plan reads unselected models and seeds from, when it defers.
fn deferred_to(
    deferral: &Bound<'_, PyAny>,
    available: &AvailableUpstreams,
) -> PyResult<Option<String>> {
    if available.deferred_relations.is_none() {
        return Ok(None);
    }
    native_field(deferral, "defer_to")?.extract()
}

/// The active target, its clone or deferral origins, and the clone `--to` it needs (also when the default target is unknown).
fn missing_upstream_fixes(
    runtime: &Bound<'_, PyAny>,
    project: &Bound<'_, PyAny>,
    deferred_to: Option<String>,
) -> PyResult<MissingUpstreamFixes> {
    let target: Option<String> = native_field(project, "effective_target_name")?.extract()?;
    if deferred_to.is_some() {
        return Ok(MissingUpstreamFixes {
            target,
            deferred_to,
            ..MissingUpstreamFixes::default()
        });
    }
    let py: Python<'_> = runtime.py();
    let configs = (
        runtime_field(runtime, |runtime| &runtime.project_config)?,
        runtime_field(runtime, |runtime| &runtime.local_config)?,
    );
    let Some(active) = target.clone() else {
        return Ok(MissingUpstreamFixes::default());
    };
    if configs.0.is_none() || configs.1.is_none() {
        return Ok(MissingUpstreamFixes {
            target,
            ..MissingUpstreamFixes::default()
        });
    }
    let allows = |name: &str, flag: &str| -> PyResult<bool> {
        let config = resolve_target_config(py, &configs.0, &configs.1, &PyString::new(py, name))?;
        native_field(&native_field(&config, "clone")?, flag)?.is_truthy()
    };
    let mut origins: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut deferrable: Vec<String> = Vec::new();
    if allows(&active, "allow_as_clone_destination")? {
        for (config, project_targets) in [(&configs.0, true), (&configs.1, false)] {
            for name in native_field(config, "targets")?.try_iter()? {
                let name: String = name?.extract()?;
                if name != active && allows(&name, "allow_as_clone_origin")? {
                    if project_targets {
                        deferrable.push(name.clone());
                    }
                    origins.insert(name);
                }
            }
        }
    }
    let default: Option<String> =
        match resolve_target_name(py, &configs.0, &configs.1, &py.None().into_bound(py)) {
            Ok(name) => name.map(|name| name.to_string()),
            Err(_) => None,
        };
    Ok(MissingUpstreamFixes {
        deferred_to: None,
        clone_to: (default.as_deref() != Some(active.as_str())).then(|| active.clone()),
        target,
        origins: origins.into_iter().collect(),
        deferrable,
    })
}
