//! Selected-scope buildability validation before change planning.

use crate::planner_phases::_helpers::inputs::contexts::class_field;
use crate::planner_phases::_helpers::inputs::runtime::runtime_field;
use crate::planner_phases::_helpers::inputs::runtime::warehouse_field;
use crate::planner_phases::constants::MISSING_UPSTREAM_CODE;
use crate::planner_warehouse::main::sources::check_selected_source_tables_exist::check_selected_source_tables_exist;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::planner_identity::main::missing_upstream_message::missing_upstream_message;
use sqlbuild_analysis::planner_identity::main::missing_upstreams::missing_upstreams;
use sqlbuild_analysis::planner_identity::models::{AvailableUpstreams, GraphKey};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::compiled_views::main::project::seeds::seeds;
use sqlbuild_python_project::graph::main::nodes::native_key::native_key;
use sqlbuild_python_project::planner_identity::main::execution_origins::execution_origins;
use sqlbuild_python_project::planner_phases::main::planner_error::planner_error;
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
        let message = missing_upstream_message(&missing, &origins);
        return Err(planner_error(
            py,
            &message,
            (Some(MISSING_UPSTREAM_CODE), None),
        ));
    }
    check_selected_source_tables_exist(
        &project,
        (
            &runtime_field(runtime, |runtime| &runtime.adapter)?,
            &runtime_field(runtime, |runtime| &runtime.connection)?,
        ),
        &selected_scope,
        &warehouse_field(warehouse, |warehouse| &warehouse.inspection_relations)?,
    )
}
