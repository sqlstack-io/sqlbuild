//! Declared jobs and groups, and the settings a selection gives its nodes.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as Json;

use crate::assembly::project::types::ObjectKey;
use crate::graph::errors::SelectorError;

/// A job's `[retry]`: at most `attempts` runs in all, each rerun `delay_seconds` after the last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobRetry {
    pub attempts: u8,
    pub delay_seconds: u32,
}

/// A declared job's `[settings]` and `[retry]`, or a group's member jobs.
#[derive(Clone, Debug, PartialEq)]
pub enum DeclaredRun {
    Job {
        settings: BTreeMap<String, Json>,
        retry: Option<JobRetry>,
    },
    Group {
        members: Vec<String>,
    },
}

/// How far one job setting reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingScope {
    /// Only the job's own nodes.
    Node,
    /// A limit on how many of the job's own nodes run at once.
    JobLimit,
    /// The whole build.
    Build,
}

/// The nodes one selector token selects, less those `--exclude` removes, as the build resolves them.
pub type TokenNodes<'r> = &'r dyn Fn(&str, &[String]) -> Result<BTreeSet<ObjectKey>, SelectorError>;

/// What the jobs a selection names give each of its nodes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeJobSettings {
    /// The named jobs each node is selected through, in the order the jobs are named.
    pub node_jobs: BTreeMap<ObjectKey, Vec<String>>,
    /// Each node's node-level settings, from whichever named job sets them.
    pub values: BTreeMap<ObjectKey, BTreeMap<String, Json>>,
    /// Each node's `[retry]`, from whichever named job sets it.
    pub retries: BTreeMap<ObjectKey, JobRetry>,
    /// Each named job's limit on how many of its nodes run at once.
    pub job_limits: BTreeMap<String, i64>,
}
