use std::collections::BTreeMap;

use serde_json::Value as Json;

pub(super) struct NodeJobSettingsTestCase {
    pub(super) description: &'static str,
    pub(super) select: &'static [&'static str],
    pub(super) exclude: &'static [&'static str],
    /// Each node's named jobs, as `name -> jobs`.
    pub(super) expected_node_jobs: BTreeMap<&'static str, Vec<&'static str>>,
    /// Each node's node-level settings, as `name -> setting -> value`.
    pub(super) expected_values: BTreeMap<&'static str, BTreeMap<&'static str, Json>>,
    pub(super) expected_job_limits: BTreeMap<&'static str, i64>,
}

pub(super) struct JobSelectorFailureTestCase {
    pub(super) description: &'static str,
    pub(super) select: &'static [&'static str],
    pub(super) expected_failure: (Vec<&'static str>, &'static str, &'static str),
}

pub(super) struct NodeJobConflictTestCase {
    pub(super) description: &'static str,
    pub(super) select: &'static [&'static str],
    pub(super) expected_message: &'static str,
}

pub(super) struct PythonNodeValuesTestCase {
    pub(super) description: &'static str,
    /// The named jobs the Python node is selected through.
    pub(super) jobs: &'static [&'static str],
    pub(super) expected_values: Option<BTreeMap<&'static str, Json>>,
    pub(super) expected_conflict: Option<&'static str>,
}

pub(super) struct BuildJobSettingsTestCase {
    pub(super) description: &'static str,
    pub(super) select: &'static [&'static str],
    pub(super) expected_named: Vec<&'static str>,
    pub(super) expected_settings: Result<BTreeMap<&'static str, Json>, &'static str>,
}

/// How a setting of one scope merges across two named jobs and against the command line.
pub(super) struct JobSettingScopeTestCase {
    pub(super) description: &'static str,
    pub(super) scope: crate::job_settings::models::SettingScope,
    /// Whether two named jobs giving different values fail as a build-wide conflict.
    pub(super) expected_build_conflict: bool,
    /// Whether a named job leaving it unset fails against one that sets it.
    pub(super) expected_unset_conflict: bool,
    /// Whether a single named job's value becomes the build's.
    pub(super) expected_build_value: bool,
    /// Whether two jobs giving one SQL node different values conflict on that node.
    pub(super) expected_node_conflict: bool,
}

/// A whole-run setting's values in two named jobs, and the build's value.
pub(super) struct WholeRunTestCase {
    pub(super) description: &'static str,
    pub(super) values: [Option<Json>; 3],
    pub(super) expected_value: Option<Json>,
}
