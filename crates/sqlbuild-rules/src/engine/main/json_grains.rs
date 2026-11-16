//! SQBRPERFORMANCE102's grain facts on each model row, built only when the request selects it.

use std::collections::{BTreeMap, BTreeSet};

use crate::models::{EvaluateRequest, GrainColumn, JsonGrain, LineageColumn, ModelRow};
use crate::query_facts::constants::SNOWFLAKE_DIALECT;
use crate::rules::main::request_selects::request_selects;

const RULE_CODE: &str = "SQBRPERFORMANCE102";
const REFERENCE_KIND: &str = "ref";
const SEMI_STRUCTURED_TYPES: [&str; 3] = ["VARIANT", "OBJECT", "ARRAY"];
/// Lineage resource types and the relation-key kinds they are declared under.
const RESOURCE_KINDS: [(&str, &str); 3] =
    [("model", "ref"), ("source", "source"), ("seed", "seed")];
const NO_LINEAGE: &str = "no column lineage";
const NO_TYPES: &str = "no inferred column types";

/// Whether `request` selects SQBRPERFORMANCE102 for a dialect it runs on.
pub fn needs_json_grains(request: &EvaluateRequest) -> Result<bool, String> {
    Ok(request.dialect.eq_ignore_ascii_case(SNOWFLAKE_DIALECT)
        && request_selects(request, |rule| rule.code == RULE_CODE)?)
}

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
    let grains: Vec<Option<JsonGrain>> = rows
        .iter()
        .zip(lineage)
        .map(|(row, lineage)| {
            let model_keys: &Vec<Vec<String>> =
                keys.get(&format!("{REFERENCE_KIND}:{}", row.name))?;
            let readers: Vec<String> = readers
                .get(row.name.as_str())
                .map(|names| names.iter().cloned().collect())
                .unwrap_or_default();
            Some(grain(row, model_keys.clone(), readers, lineage, keys))
        })
        .collect();
    for (row, grain) in rows.iter_mut().zip(grains) {
        row.json_grain = grain;
    }
}

fn grain(
    row: &ModelRow,
    model_keys: Vec<Vec<String>>,
    readers: Vec<String>,
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
        let inputs: BTreeSet<String> = column
            .sources
            .iter()
            .filter_map(|(resource, name)| {
                let kind: &str = RESOURCE_KINDS
                    .iter()
                    .find(|(known, _)| known == resource)?
                    .1;
                Some(format!("{kind}:{name}"))
            })
            .collect();
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

/// `VARIANT`, `OBJECT` or `ARRAY`, alone or with parameters such as `ARRAY(INT)`.
fn semi_structured(data_type: &str) -> bool {
    let upper: String = data_type.trim().to_ascii_uppercase();
    SEMI_STRUCTURED_TYPES.iter().any(|kind| {
        upper
            .strip_prefix(kind)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(['(', '<', ' ']))
    })
}
