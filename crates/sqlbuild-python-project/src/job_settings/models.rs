//! Models of named job settings.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::job_settings::types::PinnedJobs;

/// A frozen ad-hoc build's project directory and target, pinned jobs, the `job:` tokens naming them, their build-wide flags and the settings its command line gives.
#[derive(Clone, Debug)]
pub struct PinnedBuild {
    pub directory: (std::path::PathBuf, Option<String>),
    pub jobs: PinnedJobs,
    pub tokens: Vec<String>,
    pub flags: Vec<String>,
    pub given: BTreeSet<String>,
}

/// A frozen run's settings, the command-line flags they become, and the job a job's build names.
#[derive(Debug)]
pub struct FrozenRunFlags {
    pub settings: BTreeMap<String, Value>,
    pub flags: Vec<String>,
    pub job_build: Option<String>,
}
