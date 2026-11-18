//! Run one command with only one job declared, as its frozen run pins it.

use crate::compiled_project::_helpers::job_catalog;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Run one command with only `job` declared, as its frozen run pins it: its nodes and settings.
#[inline]
pub fn with_frozen_job<T>(
    directory: (PathBuf, Option<String>),
    frozen: (&str, Vec<String>, BTreeMap<String, Value>),
    run: impl FnOnce() -> T,
) -> T {
    job_catalog::with_frozen_job(directory, frozen, run)
}
