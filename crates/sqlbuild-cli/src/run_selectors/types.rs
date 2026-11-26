//! The past runs a command reads, by the run ID as typed.

use std::collections::BTreeMap;

use crate::run_selectors::models::PastRunKeys;

/// Each past run by the ID as typed.
pub type PastRunKeysById = BTreeMap<String, PastRunKeys>;
