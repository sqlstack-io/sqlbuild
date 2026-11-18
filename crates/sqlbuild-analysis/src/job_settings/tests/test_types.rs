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

pub(super) struct BuildJobSettingsTestCase {
    pub(super) description: &'static str,
    pub(super) select: &'static [&'static str],
    pub(super) expected_named: Vec<&'static str>,
    pub(super) expected_settings: Result<BTreeMap<&'static str, Json>, &'static str>,
}
