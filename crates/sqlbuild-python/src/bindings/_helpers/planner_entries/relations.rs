//! Relation locations and source metadata for plan entry construction.

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::compiled_views::audit_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::graph::keys::native_key;
use crate::bindings::_helpers::planner_entries::models::{
    PlannerRelationsContext, RelationsRequest,
};
use crate::bindings::_helpers::planner_entries::values::planner_model;
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_resolution::locations::build_function_locations;
use crate::bindings::_helpers::planner_resolution::locations::build_model_locations;
use crate::bindings::_helpers::planner_resolution::locations::build_seed_locations;
use crate::bindings::_helpers::planner_resolution::refs::apply_deferred_locations;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::planner_scope::sources::source_load_map;
use crate::bindings::_helpers::planner_scope::values::keys;
use crate::bindings::_helpers::planner_strategy::audit_entries::audit_is_selected;
use crate::bindings::_helpers::planner_warehouse::semantic_sources::semantic_source_columns;
use crate::bindings::_helpers::planner_warehouse::source_columns::gather_source_inspection;
use crate::bindings::_helpers::planner_warehouse::source_deferral::build_source_read_map;
use crate::bindings::_helpers::planner_warehouse::source_deferral::with_declared_source_reads;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_values;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use std::collections::HashSet;

/// Relation keys needed to render selected audits without selecting those relations.
fn selected_audit_relation_keys(
    project: &Bound<'_, PyAny>,
    selected_keys: &HashSet<ObjectKey>,
) -> PyResult<HashSet<ObjectKey>> {
    let mut keys: HashSet<ObjectKey> = HashSet::new();
    for audit in project_reads::audits(project)?.try_iter()? {
        let audit = audit?;
        if audit_is_selected(&audit, selected_keys)? {
            for dependency in audit_reads::scope_deps(&audit)?.try_iter()? {
                keys.insert(native_key(&dependency?)?);
            }
        }
    }
    Ok(keys)
}

/// Reuse already-gathered source columns for the read map, or gather them once.
fn source_warehouse_columns<'py>(
    (project, adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    source_read_map: &Bound<'py, PyAny>,
    known_source_columns: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py = project.py();
    if !known_source_columns.is_none() {
        let known = PyDict::new(py);
        for name in source_read_map.try_iter()? {
            let name = name?;
            if known_source_columns.contains(&name)? {
                known.set_item(&name, known_source_columns.get_item(&name)?)?;
            }
        }
        let names = PyFrozenSet::new(py, known.keys())?.into_any();
        return Ok((known.into_any(), names));
    }
    let entries = PyTuple::new(
        py,
        mapping_values(source_read_map)?
            .try_iter()?
            .collect::<PyResult<Vec<_>>>()?,
    )?;
    let gathered = gather_source_inspection(
        project,
        adapter,
        connection,
        entries.into_any().extract::<Option<Bound<PyAny>>>()?,
    )?;
    Ok((gathered.get_item(0)?, gathered.get_item(1)?))
}

/// Resolve relation locations and source metadata for plan entry construction.
pub(crate) fn planner_relations_context<'py>(
    request: RelationsRequest<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let RelationsRequest {
        project,
        adapter,
        connection,
        scope,
        deferral,
        project_config,
        local_config,
        known_source_columns,
        relation_keys,
    } = request;
    let py = project.py();
    let deferral = if deferral.is_none() {
        planner_model(py, RecordId::DeferralInputs, &[])?
    } else {
        deferral
    };
    let selected_keys = native_field(&scope, "selected_keys")?;
    let mut model_locations = object(
        py,
        build_model_locations(&project_reads::models(&project)?)?,
    )?;
    let mut seed_locations = object(py, build_seed_locations(&project_reads::seeds(&project)?)?)?;
    let function_locations = object(
        py,
        build_function_locations(&project_reads::functions(&project)?)?,
    )?;
    let deferred_locations = native_field(&deferral, "deferred_locations")?;
    if !deferred_locations.is_none() {
        let applied = object(
            py,
            apply_deferred_locations(
                model_locations.downcast::<PyDict>()?,
                seed_locations.downcast::<PyDict>()?,
                deferred_locations.downcast::<PyDict>()?,
                &selected_keys,
            )?,
        )?;
        model_locations = applied.get_item(0)?;
        seed_locations = applied.get_item(1)?;
    }
    let scope = scope_data(&scope)?;
    let load_keys = if relation_keys.is_none() {
        let mut relevant = selected_audit_relation_keys(&project, &scope.selected)?;
        relevant.extend(scope.selected.iter().cloned());
        relevant.into_iter().collect()
    } else {
        keys(&relation_keys)?
    };
    let source_map = object(
        py,
        source_load_map(
            &project,
            &load_keys,
            (!scope.names.is_empty()).then_some(scope.upstream.as_slice()),
        )?,
    )?;
    let deferral_enabled = native_field(&deferral, "source_deferral_enabled")?.is_truthy()?;
    let read_map = |map: &Bound<'py, PyAny>,
                    names: Option<Bound<'py, PyAny>>|
     -> PyResult<Bound<'py, PyAny>> {
        if !deferral_enabled {
            return Ok(map.clone());
        }
        let mut pairs = vec![
            ("project", project.clone()),
            ("source_map", map.clone()),
            ("selected_keys", selected_keys.clone()),
            ("project_config", project_config.clone()),
            ("local_config", local_config.clone()),
            (
                "defer_sources_to",
                native_field(&deferral, "defer_sources_to")?,
            ),
        ];
        if let Some(names) = names {
            pairs.push(("read_source_names", names));
        }
        Ok(build_source_read_map(py, Some(keywords(py, &pairs)?))?.into_any())
    };
    let source_read_map = read_map(&source_map, None)?;
    let python_source_map = object(
        py,
        with_declared_source_reads(
            &project,
            source_map.downcast::<PyDict>()?,
            &scope.field(py, "python_read_source_names")?,
        )?,
    )?;
    let python_names = PyFrozenSet::new(
        py,
        python_source_map
            .try_iter()?
            .collect::<PyResult<Vec<_>>>()?,
    )?
    .into_any();
    let python_source_read_map = read_map(&python_source_map, Some(python_names))?;
    let (columns, listed_source_names) = source_warehouse_columns(
        (&project, &adapter, &connection),
        &source_read_map,
        &known_source_columns,
    )?;
    let semantic_columns = semantic_source_columns(
        py,
        Some(keywords(
            py,
            &[
                ("project", project.clone()),
                ("adapter", adapter.clone()),
                ("connection", connection),
                ("source_read_map", source_read_map.clone()),
                ("selected_keys", selected_keys),
                ("columns", columns),
            ],
        )?),
    )?
    .into_any();
    Ok(Bound::new(
        py,
        PlannerRelationsContext {
            model_locations: model_locations.unbind(),
            seed_locations: seed_locations.unbind(),
            function_locations: function_locations.unbind(),
            source_map: source_map.unbind(),
            source_read_map: source_read_map.unbind(),
            python_source_read_map: python_source_read_map.unbind(),
            source_warehouse_columns: semantic_columns.unbind(),
            star_exclude_keyword: adapter.call_method0("star_exclude_keyword")?.unbind(),
            listed_source_names: listed_source_names.unbind(),
        },
    )?
    .into_any())
}

/// Resolve relation locations and source metadata for plan entry construction.
#[pyfunction]
#[pyo3(signature = (**kwargs))]
fn build_planner_relations_context<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "build_planner_relations_context",
        kwargs,
        (
            &["project", "adapter", "connection", "scope"],
            &[
                "deferral",
                "project_config",
                "local_config",
                "known_source_columns",
                "relation_keys",
            ],
        ),
    )?;
    planner_relations_context(RelationsRequest {
        project: args.value("project")?,
        adapter: args.value("adapter")?,
        connection: args.value("connection")?,
        scope: args.value("scope")?,
        deferral: args.value("deferral")?,
        project_config: args.value("project_config")?,
        local_config: args.value("local_config")?,
        known_source_columns: args.value("known_source_columns")?,
        relation_keys: args.value("relation_keys")?,
    })
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_planner_relations_context, module)?)
}
