//! Declared jobs and groups, and the settings a selection gives its nodes.

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::assembly::project::types::ObjectKey;

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

/// How far one job setting reaches, which also decides how named jobs' values merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingScope {
    /// Only the job's own nodes; two jobs giving one node different values conflict, unset not.
    Node,
    /// A limit on how many of the job's own nodes run at once; each job keeps its own.
    JobLimit,
    /// The whole build; every named job must set it the same way, unset differing from set.
    Build,
    /// Not a job setting: the run request owns it.
    RunRequest,
}

/// When a group's jobs must agree on a build-wide setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupAgreement {
    /// Already while each member runs as its own build, as it changes the compiled SQL.
    EachBuild,
    /// Once a group runs as one build.
    OneBuild,
}

/// One build option as a job setting: how far it reaches and how named jobs merge it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobSettingSpec {
    /// The setting's name, the build parser's destination of its options.
    pub name: &'static str,
    /// The command-line options that give it, which replace every named job's value.
    pub options: &'static [&'static str],
    pub scope: SettingScope,
    /// Whether a Python node takes it; the other node settings act on SQL nodes only.
    pub python_nodes: bool,
    /// Whether one value every named job shares becomes the whole build's, setup included.
    pub whole_run: bool,
    /// The values its command-line options can give it.
    pub command_line: CommandLineValues,
    /// For a node setting, what applies it to the plan.
    pub consumer: Option<NodeConsumer>,
    pub group: GroupAgreement,
}

/// The values a setting's command-line options can give it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandLineValues {
    /// Any value it takes, or both `true` and `false`.
    Any,
    /// Only `true`, as a flag such as `--full-refresh`.
    OnlyTrue,
    /// Only `false`, as a flag such as `--no-tests`.
    OnlyFalse,
}

/// What applies a node setting to the planned build.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeConsumer {
    /// The planner's `full_refresh_models` and `full_refresh_functions` overrides.
    RefreshOverrides,
    /// The plan's SQL test entries, dropping the tests a job turns off.
    TestEntries,
    /// Each audit entry's `ignore_failure`.
    AuditEntries,
    /// The warehouse each node runs on.
    NodeWarehouses,
}

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

/// What the named jobs give a build once the command line's own settings replace theirs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResolvedJobSettings {
    /// The declared jobs less the settings the command line gives.
    pub declared: BTreeMap<String, DeclaredRun>,
    /// The jobs the selectors name.
    pub named: Vec<String>,
    /// The build-wide settings the named jobs agree on.
    pub build: BTreeMap<String, Json>,
    /// The whole-run node settings every named job sets to one value.
    pub whole_run: BTreeMap<String, Json>,
    /// Each selected node's settings and the jobs' limits, when the nodes were resolved.
    pub nodes: Option<NodeJobSettings>,
}
