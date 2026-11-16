//! Keys proving one row per key by `kind:name`; seeds in `one_row_seeds` also get the empty key.

use crate::compiled_views::_helpers::compiled_models::relation_keys::RELATION_KIND_SEED;
use crate::compiled_views::_helpers::compiled_models::relation_keys::RELATION_KIND_SOURCE;
use crate::compiled_views::_helpers::compiled_models::relation_keys::declared_relation;
use crate::compiled_views::_helpers::compiled_models::relation_keys::source_view_key_row;
use crate::compiled_views::main::compiled_models::entry_relation_key_row::entry_relation_key_row;
use crate::compiled_views::models::RelationKeyRow;
use crate::compiled_views::types::RelationKeys;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_rules::relation_keys::main::declared_relation_keys::declared_relation_keys as derive_keys;
use sqlbuild_rules::relation_keys::models::DeclaredRelation;
use std::collections::BTreeMap;

/// Keys proving one row per key by `kind:name`; seeds in `one_row_seeds` also get the empty key.
pub fn relation_keys<'py>(
    py: Python<'py>,
    mut rows: Vec<RelationKeyRow>,
    (seeds, sources): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    one_row_seeds: &[String],
) -> PyResult<RelationKeys> {
    for (kind, entries) in [(RELATION_KIND_SEED, seeds), (RELATION_KIND_SOURCE, sources)] {
        for entry in entries.try_iter()? {
            let entry: Bound<'py, PyAny> = entry?;
            if let Some(row) = source_view_key_row(&entry) {
                rows.push(row?);
                continue;
            }
            let name: String = native_field(&entry, "name")?.extract()?;
            rows.push(entry_relation_key_row(
                kind,
                name,
                &PyDict::new(py),
                &entry,
            )?);
        }
    }
    let mut relations: Vec<DeclaredRelation> = rows.into_iter().map(declared_relation).collect();
    for relation in &mut relations {
        relation.at_most_one_row =
            relation.kind == RELATION_KIND_SEED && one_row_seeds.contains(&relation.name);
    }
    let keys: BTreeMap<String, Vec<Vec<String>>> = py.detach(|| derive_keys(&relations));
    Ok(keys.into_iter().collect())
}
