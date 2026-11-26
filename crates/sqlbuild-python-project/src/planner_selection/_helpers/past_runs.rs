//! The project's audits and the past runs a command's selectors name, attached to its graph for planning.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::constants::{AUDIT_KIND, PLANNER_DEFAULT_CODE};
use sqlbuild_analysis::graph::errors::SelectorError;
use sqlbuild_analysis::graph::main::attach_audits_and_runs::attach_audits_and_runs;
use sqlbuild_analysis::graph::main::run_selector_ids::run_selector_ids;
use sqlbuild_analysis::graph::models::{ProjectGraph, RunResults};
use sqlbuild_cli::run_selectors::main::read_past_run_keys::read_past_run_keys;
use sqlbuild_cli::run_selectors::models::{PastRunKeys, PastRunSource};
use sqlbuild_cli::run_selectors::types::PastRunKeysById;

use crate::compiled_views::main::project::audits::audits;
use crate::resource_views::main::audit_reads::name::name;
use sqlbuild_cli::run_selectors::main::names_audits_or_runs::names_audits_or_runs;

thread_local! {
    /// Where the running command's run selectors read past runs from, `None` for commands that select neither audits nor past runs; per thread, as the job scope is.
    static PAST_RUN_SOURCE: RefCell<Option<PastRunSource>> = const { RefCell::new(None) };
    /// The past runs the running command has read, so each is read once however many times its selectors resolve.
    static READ_RUNS: RefCell<PastRunKeysById> = const { RefCell::new(PastRunKeysById::new()) };
}

/// Run `run` with `source` as where its run selectors read past runs, restoring the enclosing one after.
pub(crate) fn with_past_run_source<T>(source: Option<PastRunSource>, run: impl FnOnce() -> T) -> T {
    let previous = PAST_RUN_SOURCE.with(|current| current.replace(source));
    let read = READ_RUNS.with(|current| current.replace(PastRunKeysById::new()));
    let result: T = run();
    PAST_RUN_SOURCE.with(|current| *current.borrow_mut() = previous);
    READ_RUNS.with(|current| *current.borrow_mut() = read);
    result
}

/// The past runs `ids` name, read from `source` once per command.
fn read_runs(source: &PastRunSource, ids: &BTreeSet<String>) -> Result<PastRunKeysById, String> {
    let unread: BTreeSet<String> = READ_RUNS.with(|read| {
        ids.iter()
            .filter(|id| !read.borrow().contains_key(*id))
            .cloned()
            .collect()
    });
    if !unread.is_empty() {
        let fresh = read_past_run_keys(source, &unread)?;
        READ_RUNS.with(|read| read.borrow_mut().extend(fresh));
    }
    Ok(READ_RUNS.with(|read| {
        let read = read.borrow();
        ids.iter()
            .filter_map(|id| Some((id.clone(), read.get(id)?.clone())))
            .collect()
    }))
}

/// `graph` with `project`'s audits and the past runs attached when `selectors` name an audit or a run.
pub(crate) fn with_audits_and_runs<'g>(
    project: &Bound<'_, PyAny>,
    graph: &'g ProjectGraph,
    selectors: &[String],
) -> PyResult<Result<Cow<'g, ProjectGraph>, SelectorError>> {
    let Some(source) = PAST_RUN_SOURCE.with(|source| source.borrow().clone()) else {
        return Ok(Ok(Cow::Borrowed(graph)));
    };
    if !names_audits_or_runs(selectors) {
        return Ok(Ok(Cow::Borrowed(graph)));
    }
    let mut names: BTreeSet<String> = BTreeSet::new();
    for audit in audits(project)?.try_iter()? {
        names.insert(name(&audit?)?.extract()?);
    }
    let ids = run_selector_ids(selectors);
    let past = if ids.is_empty() {
        Ok(PastRunKeysById::new())
    } else {
        read_runs(&source, &ids)
    };
    let past = match past {
        Ok(past) => past,
        Err(message) => {
            return Ok(Err(SelectorError {
                code: PLANNER_DEFAULT_CODE,
                message,
                help: None,
            }));
        }
    };
    let runs: BTreeMap<String, RunResults> = past
        .iter()
        .map(|(id, keys)| (id.clone(), graph_results(keys)))
        .collect();
    Ok(Ok(Cow::Owned(attach_audits_and_runs(
        graph.clone(),
        (names, runs),
    ))))
}

fn graph_results(keys: &PastRunKeys) -> RunResults {
    RunResults {
        selected: graph_keys(&keys.selected),
        failed: graph_keys(&keys.failed),
        skipped: graph_keys(&keys.skipped),
    }
}

/// Run node keys as selector graph keys; an audit by its name.
fn graph_keys(nodes: &BTreeSet<String>) -> BTreeSet<ObjectKey> {
    nodes.iter().filter_map(|node| run_key(node)).collect()
}

fn run_key(node: &str) -> Option<ObjectKey> {
    let (kind, name) = node.split_once(':')?;
    let name = if kind == AUDIT_KIND {
        name.split_once(':').map_or(name, |(name, _)| name)
    } else {
        name
    };
    Some((kind.to_owned(), name.to_owned()))
}
