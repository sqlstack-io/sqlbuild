//! Read-only warehouse evidence gathered once for all model migrations in one plan.

use crate::planner_migrations::models::MigrationStateInspection;
use crate::planner_phases::main::inputs::runtime_reads::runtime_reads;
use std::collections::HashSet;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
use sqlbuild_state::events::models::{EventRelation, FingerprintKey, FingerprintOrder};

use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// A lowercased schema and relation name.
pub(crate) type RelationKey = (String, String);

pub(crate) fn lowered(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.call_method0("lower")?.extract()
}

pub(crate) fn schema_or_empty<'py>(value: Bound<'py, PyAny>) -> Bound<'py, PyAny> {
    if value.is_none() {
        PyString::new(value.py(), "").into_any()
    } else {
        value
    }
}

/// `((schema or "").lower(), name.lower())` of a relation or location.
pub(crate) fn relation_key(value: &Bound<'_, PyAny>) -> PyResult<RelationKey> {
    Ok((
        lowered(&schema_or_empty(native_field(value, "schema")?))?,
        lowered(&native_field(value, "name")?)?,
    ))
}

/// The requested schemas not inspected yet, sorted, each with its lowercased key.
pub(crate) fn pending<'py>(
    schemas: &Bound<'py, PyAny>,
    inspected: &HashSet<String>,
) -> PyResult<Vec<(Bound<'py, PyAny>, String)>> {
    let mut pending: Vec<(String, Bound<'py, PyAny>, String)> = Vec::new();
    for schema in schemas.try_iter()? {
        let schema: Bound<'py, PyAny> = schema?;
        let key: String = lowered(&schema)?;
        if !inspected.contains(&key) {
            pending.push((schema.extract()?, schema, key));
        }
    }
    pending.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(pending
        .into_iter()
        .map(|(_, schema, key)| (schema, key))
        .collect())
}

pub(crate) fn tuple_of<'py>(py: Python<'py>, items: &[Py<PyAny>]) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, items.iter().map(|item| item.bind(py)))
}

pub(crate) fn fingerprint_key(fingerprint: &Bound<'_, PyAny>) -> PyResult<FingerprintKey> {
    let target_name: Bound<'_, PyAny> = native_field(fingerprint, "target_name")?;
    let target: Option<EventRelation> = if target_name.is_none() {
        None
    } else {
        Some(EventRelation {
            database: native_field(fingerprint, "target_database")?.extract()?,
            schema: native_field(fingerprint, "target_schema")?.extract()?,
            name: target_name.extract()?,
        })
    };
    Ok(FingerprintKey {
        order: FingerprintOrder {
            timestamp: native_field(fingerprint, "ts")?
                .call_method0("isoformat")?
                .extract()?,
            run_id: native_field(fingerprint, "run_id")?.extract()?,
        },
        node_name: native_field(fingerprint, "node_name")?.extract()?,
        target,
    })
}

/// A fresh live-state inspection over the runtime's connection and planning database.
pub(crate) fn inspection<'py>(
    runtime: &Bound<'py, PyAny>,
    known_fingerprints: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let state = MigrationStateInspection::new(
        &runtime_reads(runtime)?,
        &crate::planner_migrations::_helpers::planning::planning_origin::planning_database(
            runtime,
        )?,
        known_fingerprints,
    )?;
    Ok(Bound::new(runtime.py(), state)?.into_any())
}
