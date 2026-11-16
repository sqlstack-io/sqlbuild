//! One line saying what the policy counts and how far it reaches, as build and plan print it.

use crate::consumption::main::reach_value::reach_value;
use sqlbuild_state::consumption::models::{ChangeKind, ChangedPolicy};

/// One line saying what the policy counts and how far it reaches, as build and plan print it.
pub fn policy_line(policy: &ChangedPolicy) -> String {
    let label = |kind: &ChangeKind| match kind {
        ChangeKind::Definition => "definition",
        ChangeKind::SourceData => "source data",
    };
    let counted: Vec<&str> = policy.counts.iter().map(label).collect();
    let uncounted: Vec<String> = [ChangeKind::Definition, ChangeKind::SourceData]
        .iter()
        .filter(|kind| !policy.counts.contains(kind))
        .map(|kind| format!(" ({} not counted)", label(kind)))
        .collect();
    format!(
        "Counting: {}{} · reach: {}",
        counted.join(", "),
        uncounted.concat(),
        reach_value(&policy.reach)
    )
}
