//! Type aliases of named job settings.

use std::collections::BTreeSet;

use crate::assembly::project::types::ObjectKey;
use crate::graph::errors::SelectorError;

/// The nodes one selector token selects, less those `--exclude` removes, as the build resolves them.
pub type TokenNodes<'r> = &'r dyn Fn(&str, &[String]) -> Result<BTreeSet<ObjectKey>, SelectorError>;
