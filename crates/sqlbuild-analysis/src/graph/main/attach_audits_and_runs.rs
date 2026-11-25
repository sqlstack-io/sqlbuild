//! Let `audit:` select these audit names, and `run:`, `failed:` and `skipped:` each past run's results, keeping keys the project still has.

use std::collections::{BTreeMap, BTreeSet};

use crate::graph::models::{ProjectGraph, RunResults};

/// Let `audit:` select `audits`, and run selectors the `runs` they name by ID as typed.
pub fn attach_audits_and_runs(
    graph: ProjectGraph,
    (audits, runs): (BTreeSet<String>, BTreeMap<String, RunResults>),
) -> ProjectGraph {
    crate::graph::_helpers::run_selectors::attach(
        ProjectGraph {
            audits: Some(audits),
            ..graph
        },
        runs,
    )
}
