//! Attach what each past run named by `run:`, `failed:` or `skipped:` selected, keeping keys the project still has.

use std::collections::BTreeMap;

use crate::graph::models::{ProjectGraph, RunResults};

/// Attach what each past run named by `run:`, `failed:` or `skipped:` selected, keeping keys the project still has.
pub fn attach_run_results(graph: ProjectGraph, runs: BTreeMap<String, RunResults>) -> ProjectGraph {
    crate::graph::_helpers::run_selectors::attach(graph, runs)
}
