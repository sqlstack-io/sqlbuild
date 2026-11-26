//! The past runs and audits a command's selectors name, attached to the publication's graph for one plan or evaluation.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::BTreeSet;

use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::constants::AUDIT_KIND;
use sqlbuild_analysis::graph::main::attach_audits_and_runs::attach_audits_and_runs;
use sqlbuild_analysis::graph::models::{ProjectGraph, RunResults};
use sqlbuild_orchestration::runs::models::PastRunResults;
use sqlbuild_orchestration::runs::types::PastRuns;

use crate::adhoc::_helpers::publication::{IndexedPublication, python_kind};
use crate::run_selectors::main::names_audits_or_runs::names_audits_or_runs;

thread_local! {
    /// The past runs of the plan or evaluation running on this thread.
    static PAST_RUNS: RefCell<PastRuns> = const { RefCell::new(PastRuns::new()) };
}

/// Run `run` with `past` as the runs its selectors read, restoring the enclosing ones after.
pub(crate) fn with_past_runs<T>(past: &PastRuns, run: impl FnOnce() -> T) -> T {
    let previous: PastRuns = PAST_RUNS.with(|current| current.replace(past.clone()));
    let result: T = run();
    PAST_RUNS.with(|current| *current.borrow_mut() = previous);
    result
}

/// `graph` with the publication's audits and the past runs attached when `selectors` name an audit or a run.
pub(crate) fn with_audits_and_runs<'g>(
    publication: &IndexedPublication,
    graph: &'g ProjectGraph,
    (selectors, include_python): (&[String], bool),
) -> Cow<'g, ProjectGraph> {
    if !names_audits_or_runs(selectors) {
        return Cow::Borrowed(graph);
    }
    let runs = PAST_RUNS.with(|past| {
        past.borrow()
            .iter()
            .map(|(id, results)| (id.clone(), graph_results(results, include_python)))
            .collect()
    });
    Cow::Owned(attach_audits_and_runs(
        graph.clone(),
        (publication.audits.keys().cloned().collect(), runs),
    ))
}

/// A past run's results as selector graph keys.
fn graph_results(results: &PastRunResults, include_python: bool) -> RunResults {
    RunResults {
        selected: graph_keys(&results.selected, include_python),
        failed: graph_keys(&results.failed, include_python),
        skipped: graph_keys(&results.skipped, include_python),
    }
}

fn graph_keys(nodes: &BTreeSet<String>, include_python: bool) -> BTreeSet<ObjectKey> {
    nodes
        .iter()
        .filter_map(|node| run_key(node, include_python))
        .collect()
}

/// A run node key as a selector graph key: an audit by its name, a Python node only when the command runs Python.
fn run_key(node: &str, include_python: bool) -> Option<ObjectKey> {
    let (kind, name) = node.split_once(':')?;
    if kind == AUDIT_KIND {
        let name = name.split_once(':').map_or(name, |(name, _)| name);
        return Some((AUDIT_KIND.to_owned(), name.to_owned()));
    }
    if python_kind(node).is_some() && !include_python {
        return None;
    }
    Some((kind.to_owned(), name.to_owned()))
}
