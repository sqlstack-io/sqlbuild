//! Type aliases of named job settings.

use std::collections::BTreeSet;

use crate::assembly::project::types::ObjectKey;
use crate::graph::errors::SelectorError;

/// The nodes one selector token selects, less those `--exclude` removes, as the build resolves them.
pub type TokenNodes<'r> = &'r dyn Fn(&str, &[String]) -> Result<BTreeSet<ObjectKey>, SelectorError>;

/// The graph, the `--select` and `--exclude` selectors and their token resolution of a build's nodes.
pub type NodeSelection<'r> = (
    &'r crate::graph::models::ProjectGraph,
    (&'r [String], &'r [String]),
    TokenNodes<'r>,
);
