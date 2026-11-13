//! What `plan --changed` previews: the nodes a build would run, hold back and skip, and why.

use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
use sqlbuild_core::cli_text::main::format_tree_connector::format_tree_connector;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_state::consumption::models::{HeldBack, NodeKey, RunReason};

use crate::bindings::_helpers::consumption::policy::{kind_value, reach_value};
use crate::bindings::_helpers::consumption::recorder::{ChangedDecision, run_reason_text};

fn node(key: &NodeKey) -> String {
    format!("{}:{}", key.kind, key.name)
}

/// The upstream a node runs for, when it runs only because an upstream runs.
fn downstream_of(reason: &RunReason) -> Option<&NodeKey> {
    match reason {
        RunReason::UpstreamRuns(upstream) => Some(upstream),
        _ => None,
    }
}

fn held_back_detail(why: &HeldBack) -> String {
    match why {
        HeldBack::RefreshedBy(jobs) => format!("job {}", jobs.join(", ")),
        HeldBack::ReadsHeldBack(key) => format!("reads {}, which is held back", key.name),
    }
}

fn section(style: CliStyle, title: &str, rows: &[(String, String)]) -> Vec<String> {
    let width: usize = rows
        .iter()
        .map(|(name, _)| name.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines: Vec<String> = vec![style.plan_section(&format!("{title} ({})", rows.len()))];
    for (index, (name, detail)) in rows.iter().enumerate() {
        lines.push(format!(
            "{} {name:<width$}  {}",
            format_tree_connector(style, index + 1 == rows.len()),
            style.muted(detail)
        ));
    }
    lines
}

/// The Changed, Downstream of changes, Held back and Unchanged sections of the text plan.
pub(crate) fn changed_text(decision: &ChangedDecision, use_color: bool) -> String {
    let style = CliStyle { use_color };
    let mut changed: Vec<(String, String)> = Vec::new();
    let mut downstream: Vec<(String, String)> = Vec::new();
    for (key, reason) in &decision.reasons {
        match downstream_of(reason) {
            Some(upstream) => {
                downstream.push((key.name.clone(), format!("reads {}", upstream.name)))
            }
            None => changed.push((key.name.clone(), run_reason_text(reason))),
        }
    }
    let held_back: Vec<(String, String)> = decision
        .held_back
        .iter()
        .map(|(key, why)| (key.name.clone(), held_back_detail(why)))
        .collect();
    let mut blocks: Vec<String> = Vec::new();
    for (title, rows) in [
        ("Changed", &changed),
        ("Downstream of changes", &downstream),
        ("Held back", &held_back),
    ] {
        if !rows.is_empty() {
            blocks.push(section(style, title, rows).join("\n"));
        }
    }
    if !decision.skipped.is_empty() {
        blocks.push(style.muted(&format!(
            "Unchanged ({}): skipped by --changed",
            decision.skipped.len()
        )));
    }
    if blocks.is_empty() {
        return style.muted("Nothing selected runs under --changed.");
    }
    blocks.join("\n\n")
}

/// The plan JSON's `changed` object: the policy, and each selected node's outcome.
pub(crate) fn changed_json<'py>(
    py: Python<'py>,
    decision: &ChangedDecision,
) -> PyResult<Bound<'py, PyDict>> {
    let result: Bound<'py, PyDict> = PyDict::new(py);
    let counts: Vec<&str> = decision
        .policy
        .counts
        .iter()
        .map(|kind| kind_value(*kind))
        .collect();
    result.set_item("counts", counts)?;
    result.set_item("reach", reach_value(&decision.policy.reach))?;
    let runs: Bound<'py, PyList> = PyList::empty(py);
    for (key, reason) in &decision.reasons {
        let entry: Bound<'py, PyDict> = PyDict::new(py);
        entry.set_item("node", node(key))?;
        entry.set_item("reason", run_reason_text(reason))?;
        entry.set_item("downstream_of", downstream_of(reason).map(node))?;
        runs.append(entry)?;
    }
    result.set_item("runs", runs)?;
    let held_back: Bound<'py, PyList> = PyList::empty(py);
    for (key, why) in &decision.held_back {
        let entry: Bound<'py, PyDict> = PyDict::new(py);
        entry.set_item("node", node(key))?;
        match why {
            HeldBack::RefreshedBy(jobs) => entry.set_item("refreshed_by", jobs)?,
            HeldBack::ReadsHeldBack(upstream) => entry.set_item("reads", node(upstream))?,
        }
        held_back.append(entry)?;
    }
    result.set_item("held_back", held_back)?;
    let unchanged: Vec<String> = decision.skipped.iter().map(node).collect();
    result.set_item("unchanged", unchanged)?;
    Ok(result)
}
