//! Gather live candidate evidence without changing adapter inspection ownership.

use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::planner_migrations::column_objects::{
    config, definition_hash, lower,
};
use crate::bindings::_helpers::planner_migrations::models::{
    ColumnCandidate, ColumnMigrationInputs, MigrationPhase,
};
use crate::bindings::_helpers::planner_migrations::projections::parse_query_shape;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::records::fields::field as native_field;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use sqlbuild_analysis::planner_identity::constants::MODEL_RESOURCE;

pub(super) fn candidates<'py>(
    (phase, columns): (&MigrationPhase<'py>, &ColumnMigrationInputs<'py>),
    dialect: &Bound<'py, PyAny>,
) -> PyResult<Vec<ColumnCandidate<'py>>> {
    let snapshot = phase.snapshot.clone();
    let py = phase.scope.py();
    let scope = scope_data(&phase.scope)?;
    let mut result: Vec<ColumnCandidate<'py>> = Vec::new();
    for key in &scope.order {
        if !scope.selected.contains(key) || key.0 != MODEL_RESOURCE {
            continue;
        }
        let Some(model) = scope.model(py, &key.1) else {
            continue;
        };
        if model.is_none() {
            continue;
        }
        let materialized = config(&model, "materialized")?;
        if !materialized.eq("incremental")? && !materialized.eq("snapshot")? {
            continue;
        }
        let name = model_reads::name_object(&model)?;
        if columns.full_refresh_model_names.contains(&name)?
            || !native_field(&snapshot, "existing_relations")?.contains(&name)?
        {
            continue;
        }
        let warehouse = native_field(&snapshot, "existing_columns")?
            .call_method1("get", (&name, PyTuple::empty(py)))?;
        let declared = PyDict::new(py);
        let schema = model_reads::schema_entry(&model)?;
        if !schema.is_none() {
            for column in native_field(&schema, "columns")?.try_iter()? {
                let column = column?;
                let origin = native_field(&column, "migrate_from")?;
                if !origin.is_none() {
                    declared.set_item(
                        native_field(&column, "name")?,
                        origin.call_method0("strip")?,
                    )?;
                }
            }
        }
        let fingerprint = native_field(&native_field(&snapshot, "fingerprints")?, "models")?
            .call_method1("get", (&name,))?;
        let changed = !fingerprint.is_none()
            && !native_field(&fingerprint, "definition_hash")?
                .eq(definition_hash(&model, &snapshot)?)?;
        if !warehouse.is_truthy()? || (declared.is_empty() && !changed) {
            continue;
        }
        let by_name = PyDict::new(py);
        for column in warehouse.try_iter()? {
            let column = column?;
            by_name.set_item(lower(&native_field(&column, "name")?)?, column)?;
        }
        let previous = if changed {
            parse_query_shape(
                py,
                &native_field(&fingerprint, "definition")?.extract::<String>()?,
                dialect.extract::<Option<String>>()?.as_deref(),
            )?
        } else {
            py.None().into_bound(py)
        };
        let current = parse_query_shape(
            py,
            &model_reads::query_sql(&model)?.extract::<String>()?,
            dialect.extract::<Option<String>>()?.as_deref(),
        )?;
        result.push(ColumnCandidate {
            model,
            warehouse: by_name,
            declared,
            fingerprint,
            previous,
            current,
        });
    }
    Ok(result)
}

pub(super) fn declared_names<'py>(
    candidate: &ColumnCandidate<'py>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let mut names: Vec<String> = Vec::new();
    for (destination, origin) in candidate.declared.iter() {
        names.push(lower(&destination)?);
        names.push(lower(&origin)?);
    }
    PyFrozenSet::new(candidate.model.py(), names)
}
