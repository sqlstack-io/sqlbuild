//! SQBRPERFORMANCE102's grain facts on each model row, built only when the request selects it.

use std::collections::{BTreeMap, BTreeSet};

use crate::engine::_helpers::json_grain::grain;
use crate::engine::constants::REFERENCE_KIND;
use crate::models::{JsonGrain, LineageColumn, ModelRow};

/// Set each keyed row's grain from its lineage (`None` when unknown), in row order, and `keys` by `kind:name`.
pub fn json_grains(
    rows: &mut [ModelRow],
    lineage: Vec<Option<Vec<LineageColumn>>>,
    keys: &BTreeMap<String, Vec<Vec<String>>>,
) {
    let mut readers: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for row in rows.iter() {
        for reference in &row.references {
            if reference.ref_kind == REFERENCE_KIND {
                readers
                    .entry(reference.ref_name.as_str())
                    .or_default()
                    .insert(row.name.clone());
            }
        }
    }
    let mut grains: Vec<Option<JsonGrain>> = Vec::with_capacity(rows.len());
    for (row, lineage) in rows.iter().zip(lineage) {
        let Some(model_keys) = keys.get(&format!("{REFERENCE_KIND}:{}", row.name)) else {
            grains.push(None);
            continue;
        };
        let row_readers: Vec<String> = readers
            .get(row.name.as_str())
            .map_or_else(Vec::new, |names| names.iter().cloned().collect());
        grains.push(Some(grain(
            row,
            (model_keys.clone(), row_readers),
            lineage,
            keys,
        )));
    }
    for (row, grain) in rows.iter_mut().zip(grains) {
        row.json_grain = grain;
    }
}
