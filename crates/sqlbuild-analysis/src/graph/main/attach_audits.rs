//! Let `audit:` select these audit names.

use std::collections::BTreeSet;

use crate::graph::models::ProjectGraph;

/// Let `audit:` select these audit names.
pub fn attach_audits(graph: ProjectGraph, audits: BTreeSet<String>) -> ProjectGraph {
    ProjectGraph {
        audits: Some(audits),
        ..graph
    }
}
