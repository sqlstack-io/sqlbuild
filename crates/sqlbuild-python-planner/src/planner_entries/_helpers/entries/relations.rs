//! Relation locations and source metadata for plan entry construction.

use crate::planner_entries::_helpers::entries::values::planner_model;
use crate::planner_entries::models::PlannerRelationsContext;
use crate::planner_entries::models::RelationsRequest;
use crate::planner_resolution::main::references::apply_deferred_locations::apply_deferred_locations;
use crate::planner_resolution::main::references::build_function_locations::build_function_locations;
use crate::planner_resolution::main::references::build_model_locations::build_model_locations;
use crate::planner_resolution::main::references::build_seed_locations::build_seed_locations;
use crate::planner_strategy::main::audit_entries::audit_is_selected::audit_is_selected;
use crate::planner_warehouse::main::sources::build_source_read_map::build_source_read_map;
use crate::planner_warehouse::main::sources::gather_source_inspection::gather_source_inspection;
use crate::planner_warehouse::main::sources::semantic_source_columns::semantic_source_columns;
use crate::planner_warehouse::main::sources::with_declared_source_reads::with_declared_source_reads;
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::native_host::types::Syntax;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_values::mapping_values;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::project::audits::audits;
use sqlbuild_python_project::compiled_views::main::project::compiled_models::compiled_models;
use sqlbuild_python_project::compiled_views::main::project::functions::functions;
use sqlbuild_python_project::compiled_views::main::project::seeds::seeds;
use sqlbuild_python_project::graph::main::nodes::native_key::native_key;
use sqlbuild_python_project::planner_phases::main::object::object;
use sqlbuild_python_project::planner_scope::main::keys::keys;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;
use sqlbuild_python_project::planner_scope::main::source_load_map::source_load_map;
use sqlbuild_python_project::resource_views::main::audit_reads::scope_deps::scope_deps;
use std::collections::HashSet;

/// Relation keys needed to render selected audits without selecting those relations.
fn selected_audit_relation_keys(
    project: &Bound<'_, PyAny>,
    selected_keys: &HashSet<ObjectKey>,
) -> PyResult<HashSet<ObjectKey>> {
    let mut keys: HashSet<ObjectKey> = HashSet::new();
    for audit in audits(project)?.try_iter()? {
        let audit = audit?;
        if audit_is_selected(&audit, selected_keys)? {
            for dependency in scope_deps(&audit)?.try_iter()? {
                keys.insert(native_key(&dependency?)?);
            }
        }
    }
    Ok(keys)
}

/// Reuse already-gathered source columns for the read map, or gather them once.
fn source_warehouse_columns<'py>(
    (project, reads): (&Bound<'py, PyAny>, Option<&Bound<'py, PlannerReads>>),
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
    let Some(reads) = reads else {
        return Ok((
            PyDict::new(py).into_any(),
            PyFrozenSet::empty(py)?.into_any(),
        ));
    };
    let entries = PyTuple::new(
        py,
        mapping_values(source_read_map)?
            .try_iter()?
            .collect::<PyResult<Vec<_>>>()?,
    )?;
    let gathered = gather_source_inspection(
        project,
        reads,
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
        dialect,
        reads,
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
    let mut model_locations = object(py, build_model_locations(&compiled_models(&project)?)?)?;
    let mut seed_locations = object(py, build_seed_locations(&seeds(&project)?)?)?;
    let function_locations = object(py, build_function_locations(&functions(&project)?)?)?;
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
        (&project, reads.as_ref()),
        &source_read_map,
        &known_source_columns,
    )?;
    let semantic_columns = semantic_source_columns(
        py,
        Some(keywords(
            py,
            &[
                ("project", project.clone()),
                (
                    "reads",
                    reads.map_or_else(|| py.None().into_bound(py), Bound::into_any),
                ),
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
            star_exclude_keyword: dialect.syntax("star_exclude_keyword")?.call0()?.unbind(),
            listed_source_names: listed_source_names.unbind(),
        },
    )?
    .into_any())
}

/// Resolve relation locations and source metadata for plan entry construction.
#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn build_planner_relations_context<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "build_planner_relations_context",
        kwargs,
        (
            &["project", "dialect", "reads", "scope"],
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
        dialect: Dialect::passed(args.value("dialect")?)?,
        reads: Some(PlannerReads::passed(args.value("reads")?)?),
        scope: args.value("scope")?,
        deferral: args.value("deferral")?,
        project_config: args.value("project_config")?,
        local_config: args.value("local_config")?,
        known_source_columns: args.value("known_source_columns")?,
        relation_keys: args.value("relation_keys")?,
    })
}
