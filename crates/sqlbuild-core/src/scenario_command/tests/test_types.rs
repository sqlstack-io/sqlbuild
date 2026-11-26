use std::path::PathBuf;

use crate::nested_progress::models::NestedChildRow;
use crate::scenario_command::errors::ScenarioCommandError;
use crate::scenario_command::models::{
    CaptureRow, LocalRuntimeInputs, NamespaceCandidates, ScenarioFile, ScenarioNamespace,
    ScenarioResultRow, ScenarioTestFlags, SnapshotLimitConfig, SnapshotLimitInputs, SnapshotLimits,
    SqlAnalysisCommand, WarehouseRuntimeInputs,
};

pub(super) struct NamespaceTestCase {
    pub(super) description: &'static str,
    pub(super) candidates: NamespaceCandidates,
    pub(super) expected_namespace: Result<ScenarioNamespace, ScenarioCommandError>,
}

pub(super) struct SelectionTestCase {
    pub(super) description: &'static str,
    pub(super) scenario_files: Vec<ScenarioFile>,
    pub(super) selectors: Vec<String>,
    pub(super) exclude: Vec<String>,
    pub(super) expected_names: Result<Vec<&'static str>, (&'static str, &'static str)>,
}

pub(super) struct FlagsTestCase {
    pub(super) description: &'static str,
    pub(super) flags: ScenarioTestFlags,
    pub(super) expected_code: Option<&'static str>,
}

pub(super) struct LimitsTestCase {
    pub(super) description: &'static str,
    pub(super) inputs: SnapshotLimitInputs,
    pub(super) config: SnapshotLimitConfig,
    pub(super) expected_limits: SnapshotLimits,
}

pub(super) struct CapturedAtTestCase {
    pub(super) description: &'static str,
    pub(super) epoch_seconds: i64,
    pub(super) expected_text: &'static str,
}

pub(super) struct ChildRowsTestCase {
    pub(super) description: &'static str,
    pub(super) row: ScenarioResultRow,
    pub(super) expected_rows: Vec<NestedChildRow>,
}

pub(super) struct RetainedTestCase {
    pub(super) description: &'static str,
    pub(super) rows: Vec<ScenarioResultRow>,
    pub(super) relations: bool,
    pub(super) expected_text: &'static str,
}

pub(super) struct NamespaceLineTestCase {
    pub(super) description: &'static str,
    pub(super) namespace: ScenarioNamespace,
    pub(super) expected_text: &'static str,
}

pub(super) struct CaptureRowTestCase {
    pub(super) description: &'static str,
    pub(super) row: CaptureRow,
    pub(super) use_color: bool,
    pub(super) refresh: bool,
    pub(super) expected_capture_text: &'static str,
    pub(super) expected_sync_text: &'static str,
}

pub(super) struct WarehouseArtifactsTestCase {
    pub(super) description: &'static str,
    pub(super) inputs: WarehouseRuntimeInputs,
    pub(super) expected_artifacts: Vec<(PathBuf, &'static str)>,
}

pub(super) struct LocalArtifactsTestCase {
    pub(super) description: &'static str,
    pub(super) inputs: LocalRuntimeInputs,
    pub(super) expected_artifacts: Vec<(PathBuf, &'static str)>,
}

pub(super) struct NamespaceTextTestCase {
    pub(super) description: &'static str,
    pub(super) namespace: ScenarioNamespace,
    pub(super) expected_header: &'static str,
    pub(super) expected_activity: &'static str,
}

pub(super) struct SqlAnalysisTestCase {
    pub(super) description: &'static str,
    pub(super) command: SqlAnalysisCommand,
    pub(super) disabled_by_flag: bool,
    pub(super) guidance: Option<(String, String)>,
    pub(super) expected_error: Option<(&'static str, &'static str)>,
}

pub(super) struct WarningTestCase {
    pub(super) description: &'static str,
    pub(super) force: bool,
    pub(super) expected_text: &'static str,
}
