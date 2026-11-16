//! One model row's SQBRPERFORMANCE102 grain: its keys, readers and copied semi-structured columns.

use std::collections::{BTreeMap, BTreeSet};

use crate::engine::constants::{NO_LINEAGE, NO_TYPES, RESOURCE_KINDS, SEMI_STRUCTURED_TYPES};
use crate::models::{GrainColumn, JsonGrain, LineageColumn, ModelRow};

pub(crate) fn grain(
    row: &ModelRow,
    (model_keys, readers): (Vec<Vec<String>>, Vec<String>),
    lineage: Option<Vec<LineageColumn>>,
    keys: &BTreeMap<String, Vec<Vec<String>>>,
) -> JsonGrain {
    let mut grain: JsonGrain = JsonGrain {
        model_keys,
        readers,
        ..JsonGrain::default()
    };
    let Some(lineage) = lineage.filter(|columns| !columns.is_empty()) else {
        grain.skipped = Some(NO_LINEAGE.to_owned());
        return grain;
    };
    if row.inferred_columns.is_empty() {
        grain.skipped = Some(NO_TYPES.to_owned());
        return grain;
    }
    for (name, data_type) in &row.inferred_columns {
        let Some(data_type) = data_type.as_deref().filter(|kind| semi_structured(kind)) else {
            continue;
        };
        let Some(column) = lineage
            .iter()
            .find(|column| column.output.eq_ignore_ascii_case(name))
            .filter(|column| !column.aggregated)
        else {
            continue;
        };
        let inputs: BTreeSet<String> = column_inputs(column);
        for input in &inputs {
            if let Some(found) = keys.get(input) {
                grain.input_keys.insert(input.clone(), found.clone());
            }
        }
        grain.columns.push(GrainColumn {
            name: name.clone(),
            data_type: Some(data_type.to_owned()),
            inputs: inputs.into_iter().collect(),
        });
    }
    grain
}

/// `kind:name` of each relation `column` reads whose resource type declares keys.
fn column_inputs(column: &LineageColumn) -> BTreeSet<String> {
    let mut inputs: BTreeSet<String> = BTreeSet::new();
    for (resource, name) in &column.sources {
        if let Some((_, kind)) = RESOURCE_KINDS.iter().find(|(known, _)| known == resource) {
            inputs.insert(format!("{kind}:{name}"));
        }
    }
    inputs
}

/// `VARIANT`, `OBJECT` or `ARRAY`, alone or with parameters such as `ARRAY(INT)`.
fn semi_structured(data_type: &str) -> bool {
    let upper: String = data_type.trim().to_ascii_uppercase();
    SEMI_STRUCTURED_TYPES
        .iter()
        .filter_map(|kind| upper.strip_prefix(kind))
        .any(|rest| rest.is_empty() || rest.starts_with(['(', '<', ' ']))
}
