//! Selected keys without audits, and the names of the audits among them.

use std::collections::BTreeSet;

use crate::assembly::project::types::ObjectKey;

/// Selected keys without audits, and the names of the audits among them.
#[must_use]
pub fn split_audit_keys(keys: Vec<ObjectKey>) -> (Vec<ObjectKey>, BTreeSet<String>) {
    crate::graph::_helpers::run_selectors::split_audits(keys)
}
