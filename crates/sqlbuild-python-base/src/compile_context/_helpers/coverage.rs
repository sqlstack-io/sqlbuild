//! Partial semantic-check reasons from per-model facts, shared by full and delta compiles.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::compile_context::models::ModelCoverage;

/// The reason a model that opts out of SQL analysis is partial.
pub(crate) const ANALYSIS_DISABLED: &str = "SQL analysis disabled";
const INVALID_DEPENDENCY: &str = "depends on invalid model";

/// Each model's partial-check reasons, in `order`, which pairs each model with its upstream.
pub(crate) fn coverage_reasons(
    order: &[(String, Vec<String>)],
    facts: &[ModelCoverage],
) -> Vec<(String, Vec<String>)> {
    let facts: HashMap<&str, &ModelCoverage> = facts
        .iter()
        .map(|model| (model.name.as_str(), model))
        .collect();
    let mut blocked_by: HashMap<&str, HashSet<&str>> = facts
        .values()
        .filter(|model| model.blocked)
        .map(|model| (model.name.as_str(), HashSet::from([model.name.as_str()])))
        .collect();
    if !blocked_by.is_empty() {
        for _ in order {
            let mut changed: bool = false;
            for (name, upstream) in order {
                let name: &str = name.as_str();
                let mut causes: HashSet<&str> = blocked_by.get(name).cloned().unwrap_or_default();
                for upstream in upstream {
                    causes.extend(
                        blocked_by
                            .get(upstream.as_str())
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                if !causes.is_empty() && blocked_by.get(name) != Some(&causes) {
                    blocked_by.insert(name, causes);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }
    let mut reasons: Vec<(String, Vec<String>)> = Vec::new();
    for (name, _) in order {
        let name: &str = name.as_str();
        let model: Option<&&ModelCoverage> = facts.get(name);
        if model.is_some_and(|model| model.disabled) {
            reasons.push((name.to_owned(), vec![ANALYSIS_DISABLED.to_owned()]));
            continue;
        }
        let mut partial: BTreeSet<String> = model
            .map(|model| model.partial.iter().cloned().collect())
            .unwrap_or_default();
        if let Some(causes) = blocked_by.get(name) {
            for cause in causes.iter().filter(|cause| **cause != name) {
                partial.insert(format!("{INVALID_DEPENDENCY} {cause}"));
            }
        }
        if !partial.is_empty() {
            reasons.push((name.to_owned(), partial.into_iter().collect()));
        }
    }
    reasons
}

/// The whole project's notable facts: `stored` with the fresh models' replaced by `fresh`.
pub(crate) fn merged_coverage(
    stored: &[ModelCoverage],
    fresh: &[ModelCoverage],
) -> Vec<ModelCoverage> {
    let rerun: HashSet<&str> = fresh.iter().map(|model| model.name.as_str()).collect();
    stored
        .iter()
        .filter(|model| !rerun.contains(model.name.as_str()))
        .chain(fresh.iter().filter(|model| model.notable()))
        .cloned()
        .collect()
}

/// Every project model with its upstream names, the fresh models' replacing the stored.
pub(crate) fn merged_order(
    stored: &[(String, Vec<String>)],
    fresh: &[(String, Vec<String>)],
) -> Vec<(String, Vec<String>)> {
    let fresh: HashMap<&str, &Vec<String>> = fresh
        .iter()
        .map(|(name, upstream)| (name.as_str(), upstream))
        .collect();
    stored
        .iter()
        .map(|(name, upstream)| {
            (
                name.clone(),
                fresh
                    .get(name.as_str())
                    .map_or(upstream, |fresh| *fresh)
                    .clone(),
            )
        })
        .collect()
}
