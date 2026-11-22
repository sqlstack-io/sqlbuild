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

pub(super) struct ProjectNameTestCase {
    pub(super) description: &'static str,
    pub(super) project_file: &'static str,
    pub(super) expected_name: Option<&'static str>,
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

/// The frozen settings of one plan request.
pub(super) struct PlanArgumentsTestCase {
    pub(super) description: &'static str,
    pub(super) settings: serde_json::Value,
    /// Whether the location refuses the settings instead of planning, and keeping, them.
    pub(super) expected_refused: bool,
}

/// One kept plan, then one request for a plan.
pub(super) struct PlanReuseTestCase {
    pub(super) description: &'static str,
    /// The kept plan's generation, copy version and whether it failed.
    pub(super) kept: (i64, &'static str, bool),
    /// The generation, copy version and selected names of the request.
    pub(super) asked: (i64, &'static str, &'static [&'static str]),
    /// How long after the kept plan started the request comes.
    pub(super) age: chrono::TimeDelta,
    /// The reused plan's timings, or `None` when the location plans again.
    pub(super) expected_timings: Option<serde_json::Value>,
}

/// Plans kept within a count and byte limit, maybe a plan of a newer copy, then one request.
pub(super) struct PlanReuseBoundTestCase {
    pub(super) description: &'static str,
    /// The most plans and bytes kept, and how many plans are kept first.
    pub(super) limits: (usize, usize, usize),
    pub(super) newer_copy: bool,
    /// The selected name and copy version of the request.
    pub(super) asked: (&'static str, &'static str),
    pub(super) expected_reused: bool,
}

/// One `sqb plan --json` output, whether it exited as a plan, and the first model the server receives.
pub(super) struct PlanDocumentTestCase {
    pub(super) description: &'static str,
    pub(super) output: Vec<u8>,
    pub(super) planned: bool,
    pub(super) expected_first_model: serde_json::Value,
    /// How many models' diffs travel beside the document, and the first one's query diff.
    pub(super) expected_diffs: (usize, Option<Vec<String>>),
}

/// One failed metadata read, whether it was a page-size failure, and the statistics page size afterwards.
pub(super) struct MetadataRetryTestCase {
    pub(super) description: &'static str,
    pub(super) page_size: bool,
    pub(super) failed_kind: WarehouseHistoryKind,
    pub(super) acknowledged_pages: usize,
    pub(super) expected_statistics_limit: usize,
}

/// Failed reads of one table (true when page-size) around a sync request, and what follows them.
pub(super) struct MetadataBackoffTestCase {
    pub(super) description: &'static str,
    pub(super) failures: Vec<bool>,
    pub(super) failures_after_request: Vec<bool>,
    pub(super) expected_waits: Vec<Option<u64>>,
    pub(super) expected_limit: usize,
    pub(super) expected_halted: bool,
    /// Whether the sync pages right after the request, and after the last failure.
    pub(super) expected_paging: (bool, bool),
}

/// A sync that applied a page and stopped at statistics on a server whose clock runs an hour ahead.
pub(super) struct StoppedPassTestCase {
    pub(super) description: &'static str,
    /// When the last run ended, in seconds after the stop by the server's clock.
    pub(super) run_ended_after_stop: i64,
    /// Whether it resumed, whether its pass restarts, and whether its completion plans what changed.
    pub(super) expected_outcome: (bool, bool, bool),
}

/// A sync stopped by failed reads, its safety interval and how soon the safety sync is due.
pub(super) struct SafetyResumeTestCase {
    pub(super) description: &'static str,
    pub(super) safety_seconds: u64,
    pub(super) due_in: std::time::Duration,
    /// History reads wake billed compute and the safety interval is the default.
    pub(super) billed: bool,
    /// A run ended after the sync stopped.
    pub(super) ran: bool,
    /// Whether it resumed and pages, the next failure's wait, and whether that stopped it again.
    pub(super) expected_outcome: (bool, bool, Option<std::time::Duration>, bool),
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

pub(crate) struct NewestVersionsTestCase {
    pub(crate) description: &'static str,
    pub(crate) kind: WarehouseHistoryKind,
    pub(crate) incremental: bool,
    pub(crate) server_limit: u16,
    pub(crate) expected_newest_versions: Option<u16>,
}

pub(crate) struct ReadFailureTestCase {
    pub(crate) description: &'static str,
    pub(crate) script: std::path::PathBuf,
    pub(crate) kind: WarehouseHistoryKind,
    pub(crate) expected: fn(&crate::native_commands::models::ReadFailure) -> bool,
    pub(crate) expected_line: &'static str,
}

pub(super) struct BilledDeferralTestCase {
    pub(super) description: &'static str,
    /// What the publication's connection check reports for the adapter.
    pub(super) wakes_compute: bool,
    /// Whether the sync stopped, whether it defers safety syncs, and whether one is due once its interval passed.
    pub(super) expected_outcome: (bool, bool, bool),
}

#[derive(Debug)]
pub(super) struct SyncCompletionTestCase {
    pub(super) description: &'static str,
    pub(super) acknowledged_scans: usize,
    pub(super) more_pages: bool,
    pub(super) failure: Option<sqlbuild_orchestration::platform::models::WarehouseSyncFailure>,
    pub(super) safety_seconds: u64,
    pub(super) reads_wake_compute: bool,
    pub(super) expected_next_in_seconds: Option<i64>,
}

#[derive(Debug)]
pub(super) struct ChangedBuildNodesTestCase {
    pub(super) description: &'static str,
    pub(super) document: serde_json::Value,
    pub(super) expected_nodes: Result<(Vec<&'static str>, Vec<&'static str>), &'static str>,
}

pub(super) struct LineResetTestCase {
    pub(super) description: &'static str,
    pub(super) shared: bool,
    pub(super) terminal: bool,
    pub(super) expected_reset: &'static [u8],
}

/// One turn of named work and how long the location waits before it checks for work again.
pub(super) struct WorkCheckTestCase {
    pub(super) description: &'static str,
    pub(super) claimed: bool,
    pub(super) sync: bool,
    pub(super) expected_wait: std::time::Duration,
}

/// One failed read after earlier failures in a row, on a warehouse whose reads may wake compute.
pub(crate) struct FailedReadHealthTestCase {
    pub(crate) description: &'static str,
    pub(crate) failure: crate::native_commands::models::ReadFailure,
    pub(crate) failures_before: usize,
    pub(crate) billed: bool,
    /// Shrinks, closed cause, reported error, whether the sync stops, and seconds to the announced next sync.
    pub(crate) expected_report: (
        bool,
        Option<sqlbuild_orchestration::platform::models::WarehouseReadFailure>,
        Option<&'static str>,
        bool,
        Option<i64>,
    ),
}

/// A table's progress line after a sync read it or found it unchanged.
pub(crate) struct ScanLineTestCase {
    pub(crate) description: &'static str,
    pub(crate) index: usize,
    pub(crate) scan_rows: u64,
    pub(crate) unchanged: bool,
    pub(crate) expected_prefix: &'static str,
}

/// A row count as the progress lines write it.
pub(crate) struct RowsTextTestCase {
    pub(crate) description: &'static str,
    pub(crate) rows: u64,
    pub(crate) expected_text: &'static str,
}

/// A delivered or failed page's progress report at one kind of history.
pub(crate) struct SyncProgressTestCase {
    pub(crate) description: &'static str,
    pub(crate) index: usize,
    pub(crate) more_pages: bool,
    pub(crate) clean: bool,
    /// Position, kinds, rows and whether the sync completed.
    pub(crate) expected_progress: (u64, u64, u64, bool),
}

/// An idle location on a warehouse whose reads may wake compute, after a run or a request.
pub(crate) struct WakingIdleTestCase {
    pub(crate) description: &'static str,
    pub(crate) wakes: bool,
    pub(crate) run_ended: bool,
    pub(crate) requested: bool,
    pub(crate) expected_due: bool,
}

/// The options a changed build is frozen with.
pub(super) struct ChangedBuildSettingsTestCase {
    pub(super) description: &'static str,
    pub(super) expected_settings: serde_json::Value,
}

/// Completed syncs in order, each as (applied a page, plan reached the server), after a publication.
pub(super) struct ReplanTestCase {
    pub(super) description: &'static str,
    pub(super) syncs: Vec<(bool, bool)>,
    pub(super) expected_replans: Vec<bool>,
}

pub(crate) struct SkippedCountTestCase {
    pub(crate) description: &'static str,
    pub(crate) locations: usize,
    /// The count a read of the same kind in the first location reports after the skipped one's read.
    pub(crate) later_read: Option<u64>,
    pub(crate) expected_mirrored: Option<u64>,
}

/// A scan whose location clock runs ten minutes ahead of the server's.
pub(super) struct FastClockScanTestCase {
    pub(super) description: &'static str,
    /// Scans of the pass completed before the one whose id is checked.
    pub(super) scans_completed: usize,
    /// Whether a table altered a second after the page read is still described live.
    pub(super) expected_described_live: bool,
}

/// One admin result and what it prints without `--json`.
pub(crate) struct AdminTextTestCase {
    pub(crate) description: &'static str,
    pub(crate) done: crate::native_commands::models::AdminDone,
    pub(crate) expected_line: Option<&'static str>,
    pub(crate) expected_stdout: &'static str,
}
