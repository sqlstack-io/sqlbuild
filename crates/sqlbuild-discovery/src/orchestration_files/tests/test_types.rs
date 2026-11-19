use std::collections::BTreeMap;

use crate::orchestration_files::models::{
    FilePosition, GroupDependencies, Overlap, Positioned, RetryPolicy, ScheduleFile,
};

pub(super) struct JobReadTestCase {
    pub(super) description: &'static str,
    pub(super) text: &'static str,
    pub(super) expected_name: Positioned<String>,
    pub(super) expected_select: Positioned<String>,
    pub(super) expected_exclude: Option<&'static str>,
    pub(super) expected_overlap: Overlap,
    pub(super) expected_pools: &'static [(&'static str, u32)],
    pub(super) expected_retry: Option<RetryPolicy>,
    pub(super) expected_schedule: Option<ScheduleFile>,
    pub(super) expected_cpus: Option<serde_json::Value>,
    pub(super) expected_settings: BTreeMap<String, serde_json::Value>,
}

pub(super) struct FileErrorTestCase {
    pub(super) description: &'static str,
    pub(super) text: &'static str,
    pub(super) expected_message: &'static str,
    pub(super) expected_position: FilePosition,
    pub(super) expected_help: Option<&'static str>,
}

pub(super) struct GroupReadTestCase {
    pub(super) description: &'static str,
    pub(super) text: &'static str,
    pub(super) expected_members: Result<Vec<(String, FilePosition)>, (String, FilePosition)>,
}

pub(super) struct InventoryTestCase {
    pub(super) description: &'static str,
    pub(super) files: Vec<(&'static str, &'static str)>,
    pub(super) expected_names: Vec<(String, &'static str, String)>,
    pub(super) expected_modules: Vec<&'static str>,
    pub(super) expected_empty: bool,
}

pub(super) struct InventoryErrorTestCase {
    pub(super) description: &'static str,
    pub(super) files: Vec<(&'static str, &'static str)>,
    pub(super) expected_message: &'static str,
    pub(super) expected_help: Option<&'static str>,
}

pub(super) struct GroupDependenciesTestCase {
    pub(super) description: &'static str,
    pub(super) dependencies: &'static str,
    pub(super) expected_dependencies: Result<GroupDependencies, (&'static str, FilePosition)>,
}

pub(super) struct PoolCheckTestCase {
    pub(super) description: &'static str,
    pub(super) files: Vec<(&'static str, &'static str)>,
    pub(super) limits: sqlbuild_config::project::models::PoolLimits,
    pub(super) expected_failure: Option<(&'static str, &'static str)>,
}
