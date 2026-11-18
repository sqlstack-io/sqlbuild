//! Selected-scope buildability validation before change planning.

use crate::planner_phases::_helpers::inputs::contexts::class_field;
use crate::planner_phases::_helpers::inputs::runtime::runtime_field;
use crate::planner_phases::_helpers::inputs::runtime::warehouse_field;
use crate::planner_phases::constants::MISSING_UPSTREAM_CODE;
use crate::planner_phases::main::inputs::runtime_dialect::runtime_dialect;
use crate::planner_phases::main::inputs::runtime_reads::runtime_reads;
use crate::planner_warehouse::main::sources::check_selected_source_tables_exist::check_selected_source_tables_exist;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
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
use sqlbuild_python_project::compiled_views::main::project::seeds::seeds;
use sqlbuild_python_project::graph::main::nodes::native_key::native_key;
use sqlbuild_python_project::planner_identity::main::execution_origins::execution_origins;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;
use sqlbuild_python_project::planner_scope::models::PlannerScopeResolution;
use sqlbuild_python_project::resource_views::main::seed_reads::external::external;
use sqlbuild_python_project::resource_views::main::seed_reads::key::key;

pub(crate) fn check_selected_scope_buildability<'py>(
    runtime: &Bound<'py, PyAny>,
    scopes: &Bound<'py, PyAny>,
    warehouse: &Bound<'py, PyAny>,
    deferral: &Bound<'py, PyAny>,
) -> PyResult<()> {
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
    let deferred = native_field(deferral, "deferred_relations")?;
    let available = AvailableUpstreams {
        satisfied_keys,
        existing_relations: names(native_field(&snapshot, "existing_relations")?)?,
        deferred_relations: if deferred.is_none() {
            None
        } else {
            Some(names(deferred)?)
        },
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
    let missing = missing_upstreams(&selected, &deps, &available);
    if !missing.is_empty() {
        let origins: std::collections::HashMap<(GraphKey, GraphKey), String> =
            execution_origins(&project)?
                .into_iter()
                .map(|((key, read), label)| ((GraphKey::from(&key), GraphKey::from(&read)), label))
                .collect();
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
    )
}

/// The active target, its clone or deferral origins, and the clone `--to` it needs (also when the default target is unknown).
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

fn missing_upstream_fixes(
    runtime: &Bound<'_, PyAny>,
    project: &Bound<'_, PyAny>,
    deferred_to: Option<String>,
) -> PyResult<MissingUpstreamFixes> {
    if deferred_to.is_some() {
        return Ok(MissingUpstreamFixes {
            target: deferred_to,
            ..MissingUpstreamFixes::default()
        });
    }
    let py: Python<'_> = runtime.py();
    let target: Option<String> = native_field(project, "effective_target_name")?.extract()?;
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
        clone_to: (default.as_deref() != Some(active.as_str())).then(|| active.clone()),
        target,
        origins: origins.into_iter().collect(),
        deferrable,
    })
}
