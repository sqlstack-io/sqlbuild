use std::collections::HashMap;

use crate::lineage_command::models::{LineageIndex, LineageKey, LineageNode};

/// A key of `resource_type` named `name`.
pub(super) fn key(resource_type: &str, name: &str) -> LineageKey {
    LineageKey {
        resource_type: resource_type.to_owned(),
        name: name.to_owned(),
    }
}

/// `raw_orders -> stg_orders -> fact_orders <- waffle_types`, with model paths.
pub(super) fn orders_index() -> LineageIndex {
    let raw: LineageKey = key("source", "raw_orders");
    let stg: LineageKey = key("model", "stg_orders");
    let fact: LineageKey = key("model", "fact_orders");
    let seed: LineageKey = key("seed", "waffle_types");
    let nodes: HashMap<LineageKey, LineageNode> = [
        (stg.clone(), Some("sql/models/stg_orders.sql")),
        (fact.clone(), Some("sql/models/fact_orders.sql")),
        (raw.clone(), None),
        (seed.clone(), None),
    ]
    .into_iter()
    .map(|(key, path)| {
        (
            key.clone(),
            LineageNode {
                key,
                relative_path: path.map(str::to_owned),
                qualified_name: None,
            },
        )
    })
    .collect();
    LineageIndex {
        nodes,
        upstream: vec![
            (stg.clone(), vec![raw.clone()]),
            (fact.clone(), vec![stg.clone(), seed.clone()]),
        ],
        downstream: vec![
            (raw.clone(), vec![stg.clone()]),
            (stg.clone(), vec![fact.clone()]),
            (seed.clone(), vec![fact.clone()]),
        ],
        tags: Vec::new(),
        paths: Vec::new(),
        all_keys: [raw, stg, fact, seed]
            .into_iter()
            .map(|key| (key.name.clone(), key))
            .collect(),
    }
}
