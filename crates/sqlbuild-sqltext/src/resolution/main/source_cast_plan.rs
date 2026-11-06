//! Plan source casts without making adapter rendering decisions.

use crate::resolution::main::column_name_key::column_name_key;
use crate::resolution::models::{SourceCastPlan, SourceProjection};
use crate::resolution::types::SourceColumns;
use std::collections::{HashMap, HashSet};

/// Match declared casts to the actual source columns with the source-kind case policy.
pub fn source_cast_plan(
    declared: &SourceColumns,
    available: &SourceColumns,
    expression: bool,
) -> SourceCastPlan {
    let mut enforced: Vec<(&str, &str)> = Vec::new();
    for (name, kind) in declared {
        let Some(kind) = kind else {
            continue;
        };
        if let Some((_, previous)) = enforced.iter_mut().find(|(key, _)| *key == name) {
            *previous = kind;
        } else {
            enforced.push((name, kind));
        }
    }
    if expression {
        let names: HashMap<String, &str> = available
            .iter()
            .map(|(name, _)| (column_name_key(name), name.as_str()))
            .collect();
        let mut casts: HashMap<&str, (&str, &str)> = HashMap::new();
        for (declared, kind) in enforced {
            if let Some(actual) = names.get(&column_name_key(declared)) {
                casts.insert(*actual, (declared, kind));
            }
        }
        let projections: Vec<SourceProjection> = available
            .iter()
            .map(|(name, _)| expression_projection(name, &casts))
            .collect();
        return SourceCastPlan {
            projections,
            all_columns_cast: false,
        };
    }
    let names: HashSet<&str> = available.iter().map(|(name, _)| name.as_str()).collect();
    let keys: HashSet<String> = names.iter().map(|name| column_name_key(name)).collect();
    let projections: Vec<SourceProjection> = enforced
        .into_iter()
        .filter(|(name, _)| names.contains(name) || keys.contains(&column_name_key(name)))
        .map(|(name, kind)| SourceProjection {
            name: name.to_owned(),
            cast: Some((name.to_owned(), kind.to_owned())),
        })
        .collect();
    let all_columns_cast = projections.len() == names.len();
    SourceCastPlan {
        projections,
        all_columns_cast,
    }
}

fn expression_projection(name: &str, casts: &HashMap<&str, (&str, &str)>) -> SourceProjection {
    SourceProjection {
        name: name.to_owned(),
        cast: casts
            .get(name)
            .map(|(declared, kind)| ((*declared).to_owned(), (*kind).to_owned())),
    }
}
