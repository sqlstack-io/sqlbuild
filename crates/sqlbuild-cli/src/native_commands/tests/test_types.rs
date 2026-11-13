use sqlbuild_orchestration::warehouse_metadata::models::WarehouseHistoryKind;

use crate::argv::models::{TypeFailure, Value};
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::models::{CommandOutput, NativeCommand, RulesAction};

/// One converter applied to one command-line string.
pub(super) struct ConversionTestCase {
    pub(super) description: &'static str,
    pub(super) converter: &'static str,
    pub(super) text: &'static str,
    pub(super) expected_value: Result<Value, TypeFailure>,
    pub(super) expected_needs_python: bool,
}

/// One accepted command line, the native command it runs and its invocation services.
pub(super) struct DecisionTestCase {
    pub(super) description: &'static str,
    pub(super) argv: &'static [&'static str],
    pub(super) expected_command: Result<NativeCommand, PythonNeeded>,
    pub(super) expected_creates_project: bool,
}

/// One command line and whether its parse needs Python.
pub(super) struct ParseTestCase {
    pub(super) description: &'static str,
    pub(super) argv: &'static [&'static str],
    pub(super) expected_needs_python: bool,
}

/// One `--project-dir` string and the `pathlib.Path` it names.
pub(super) struct PathlibTestCase {
    pub(super) description: &'static str,
    pub(super) text: &'static str,
    pub(super) expected_path: &'static str,
}

/// A path beneath a real directory and what `Path.resolve()` makes of it.
pub(super) struct ResolveTestCase {
    pub(super) description: &'static str,
    pub(super) text: &'static str,
    pub(super) expected_parts: &'static [&'static str],
}

/// Project directories and files, and whether an observed command can run without Python.
pub(super) struct ProjectFactsTestCase {
    pub(super) description: &'static str,
    pub(super) directories: &'static [&'static str],
    pub(super) files: &'static [(&'static str, &'static str)],
    pub(super) expected_native: bool,
}

/// One `rules` inspection over a project and the start of what it prints.
pub(super) struct RulesTestCase {
    pub(super) description: &'static str,
    pub(super) directories: &'static [&'static str],
    pub(super) project_file: &'static str,
    pub(super) action: RulesAction,
    pub(super) selector: Option<&'static str>,
    pub(super) json: bool,
    pub(super) expected_output: Result<(&'static str, i32), PythonNeeded>,
}

/// The printed start and exit status of a command's output, for comparison.
pub(super) fn output_start(output: &CommandOutput, length: usize) -> (String, i32) {
    (
        output.stdout.chars().take(length).collect(),
        output.exit_code,
    )
}

/// Bytes a reader no longer takes and what Python's stdout buffer makes of them.
pub(super) struct LostStdoutTestCase {
    pub(super) description: &'static str,
    pub(super) total: usize,
    pub(super) expected_lost: crate::native_commands::models::LostStdout,
}

/// A directory `init` meets once the project file exists, and the start of its error.
pub(super) struct InitFailureTestCase {
    pub(super) description: &'static str,
    pub(super) blocking_file: &'static str,
    pub(super) expected_stderr_start: &'static str,
    pub(super) expected_exit_code: i32,
}

/// One `sqb plan --json` output, whether it exited as a plan, and the first model the server receives.
pub(super) struct PlanDocumentTestCase {
    pub(super) description: &'static str,
    pub(super) output: Vec<u8>,
    pub(super) planned: bool,
    pub(super) expected_first_model: serde_json::Value,
}

/// One failed metadata read and the statistics page size the sync uses afterwards.
pub(super) struct MetadataRetryTestCase {
    pub(super) description: &'static str,
    pub(super) failed_kind: WarehouseHistoryKind,
    pub(super) acknowledged_pages: usize,
    pub(super) expected_statistics_limit: usize,
}

/// A completed state-table read, the later scan of the same table, and whether that scan may skip.
pub(super) struct UnchangedScanTestCase {
    pub(super) description: &'static str,
    pub(super) read_reconciled: bool,
    pub(super) read_delivered: bool,
    pub(super) read_incremental: bool,
    pub(super) rereads_without_token: usize,
    pub(super) scan_reconciles: bool,
    pub(super) mirror_holds_rows: bool,
    pub(super) expected_token: Option<&'static str>,
}

pub(super) struct SyncCoalescingTestCase {
    pub(super) description: &'static str,
    /// Sync requests that arrive while one sync is in progress, such as one per CLI build.
    pub(super) requests: usize,
    pub(super) run_ends_during_sync: bool,
    /// Seconds between safety syncs; `0` turns them off.
    pub(super) safety_seconds: u64,
    /// Seconds until the safety interval passes, set while the sync runs; `0` passes during it.
    pub(super) safety_due_after_seconds: u64,
    pub(super) expected_follow_ups: usize,
}

pub(super) struct SafetySyncSettingTestCase {
    pub(super) description: &'static str,
    pub(super) flag: Option<&'static str>,
    pub(super) environment: Option<&'static str>,
    pub(super) project: Option<u64>,
    pub(super) expected_seconds: Result<u64, &'static str>,
}

/// An idle location after one complete sync, and whether its safety interval makes a sync due.
pub(super) struct SafetySyncIdleTestCase {
    pub(super) description: &'static str,
    /// Seconds between safety syncs; `0` turns them off.
    pub(super) safety_seconds: u64,
    pub(super) safety_due_after_seconds: u64,
    pub(super) expected_sync_due: bool,
}
