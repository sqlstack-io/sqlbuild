//! `audit:` and the past-run selectors `run:`, `failed:` and `skipped:` over what a command attached to the graph.

use std::collections::{BTreeMap, BTreeSet};

use crate::assembly::project::types::ObjectKey;
use crate::graph::_helpers::selectors::{Keys, error, matching_names, parse};
use crate::graph::constants::{
    AUDIT_KIND, FAILED_KIND, INTERSECTION_SEPARATOR, KIND_SEPARATOR, RUN_SELECTOR_KINDS,
    SKIPPED_KIND, UNMAPPED_KIND_CODE, UNMAPPED_KINDS,
};
use crate::graph::errors::SelectorError;
use crate::graph::models::{ParsedSelector, ProjectGraph, RunResults};

type Resolved = Result<Keys, SelectorError>;

/// The audits `value` names or matches, as `("audit", name)` keys.
pub(crate) fn audit(graph: &ProjectGraph, value: &str) -> Resolved {
    let Some(audits) = &graph.audits else {
        return Err(error(
            UNMAPPED_KIND_CODE,
            format!(
                "selector '{AUDIT_KIND}{KIND_SEPARATOR}{value}' selects an audit; \
                 only plan, build, test, audit and check select audits"
            ),
        ));
    };
    Ok(matching_names(value, audits.iter(), "audit")?
        .into_iter()
        .map(|name| (AUDIT_KIND.to_owned(), name.clone()))
        .collect())
}

/// What the run `value` names selected, didn't finish or skipped, by `kind`.
pub(crate) fn run_results(graph: &ProjectGraph, (kind, value): (&str, &str)) -> Resolved {
    let Some(results) = graph.runs.as_ref().and_then(|runs| runs.get(value)) else {
        return Err(error(
            UNMAPPED_KIND_CODE,
            format!(
                "selector '{kind}{KIND_SEPARATOR}{value}' selects from a past run; \
                 only plan, build, test, audit and check read past runs"
            ),
        ));
    };
    Ok(match kind {
        FAILED_KIND => results.failed.clone(),
        SKIPPED_KIND => results.skipped.clone(),
        _ => results.selected.clone(),
    })
}

/// The run IDs, as typed, that `run:`, `failed:` and `skipped:` parts of `selectors` name.
pub(crate) fn run_ids(selectors: &[String]) -> BTreeSet<String> {
    selectors
        .iter()
        .flat_map(|selector| selector.split(char::is_whitespace))
        .flat_map(|token| token.split(INTERSECTION_SEPARATOR))
        .filter(|part| !part.is_empty())
        .filter_map(|part| match parse(part) {
            Ok(ParsedSelector::Kind { kind, value, .. })
                if RUN_SELECTOR_KINDS.contains(&kind.as_str()) =>
            {
                Some(value)
            }
            _ => None,
        })
        .collect()
}

/// `graph` with each run's results kept to the SQL keys and audits it still has; Python nodes pass, as a job's do, for the caller to keep or leave out.
pub(crate) fn attach(graph: ProjectGraph, runs: BTreeMap<String, RunResults>) -> ProjectGraph {
    let known = |key: &ObjectKey| {
        if key.0 == AUDIT_KIND {
            return graph
                .audits
                .as_ref()
                .is_some_and(|audits| audits.contains(&key.1));
        }
        UNMAPPED_KINDS.contains(&key.0.as_str()) || graph.key_named(&key.1) == Some(key)
    };
    let kept = |keys: &BTreeSet<ObjectKey>| keys.iter().filter(|key| known(key)).cloned().collect();
    let runs: BTreeMap<String, RunResults> = runs
        .iter()
        .map(|(id, results)| {
            (
                id.clone(),
                RunResults {
                    selected: kept(&results.selected),
                    failed: kept(&results.failed),
                    skipped: kept(&results.skipped),
                },
            )
        })
        .collect();
    ProjectGraph {
        runs: Some(runs),
        ..graph
    }
}
