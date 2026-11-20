//! Models of named job settings.

use std::collections::BTreeMap;

use serde_json::Value;

/// A frozen run's settings, the command-line flags they become, and the job a job's build names.
#[derive(Debug)]
pub struct FrozenRunFlags {
    pub settings: BTreeMap<String, Value>,
    pub flags: Vec<String>,
    pub job_build: Option<String>,
}
