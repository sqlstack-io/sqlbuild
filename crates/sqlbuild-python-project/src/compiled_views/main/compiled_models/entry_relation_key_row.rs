//! The key row of a seed, source or hand-built model from its config values and schema entry.

use crate::compiled_project::main::python_column_facts::python_column_facts;
use crate::compiled_views::_helpers::compiled_models::relation_keys::declared_specs;
use crate::compiled_views::_helpers::compiled_models::relation_keys::relation_key_row;
use crate::compiled_views::main::compiled_models::audit_key_rows::audit_key_rows;
use crate::compiled_views::models::RelationKeyRow;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyDict;
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;

/// The key row of a seed, source or hand-built model from its config values and schema entry.
pub fn entry_relation_key_row(
    kind: &str,
    name: String,
    values: &Bound<'_, PyDict>,
    entry: &Bound<'_, PyAny>,
) -> PyResult<RelationKeyRow> {
    if entry.is_none() {
        return relation_key_row(kind, name, values, None);
    }
    let columns: Vec<SchemaColumnFact> =
        python_column_facts(&declared_specs(entry, "columns")?, None)?;
    let audits: Bound<'_, PyAny> = declared_specs(entry, "audits")?;
    relation_key_row(
        kind,
        name,
        values,
        Some((&columns, audit_key_rows(&audits)?)),
    )
}
