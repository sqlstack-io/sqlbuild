//! Python-free commands, the invocation facts they run with and what they print.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use sqlbuild_cache::project_snapshot::models::{PathStamp, SnapshotRules};
use sqlbuild_orchestration::credentials::models::CredentialScope;
use sqlbuild_orchestration::declarations::models::TriggerChange;
use sqlbuild_orchestration::platform::models::{LocationWorkRequest, WarehouseReadFailure};
use sqlbuild_orchestration::runs::models::{JobRunRequest, RerunRequest};
use sqlbuild_orchestration::sensors::models::SensorEventChoice;
use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_PAGE_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    MetadataBatch, StatisticsRead, WarehouseHistoryFailure, WarehouseHistoryKind,
    WarehouseHistoryPage, WarehouseStateLocation,
};
use uuid::{NoContext, Timestamp, Uuid};

use crate::argv::models::Namespace;
use crate::native_commands::constants::{METADATA_FAILURE_LIMIT, METADATA_POLL_SECONDS};
use crate::native_commands::types::LocationPublication;

/// Privileged local operations require direct ownership of the control database.
pub(crate) enum ControlAdmin {
    CreateLocation(String),
    CreatePrincipal(String),
    BootstrapUser(String),
    IssueToken(CredentialScope, DateTime<Utc>),
    RevokeToken(Uuid),
    RotateToken(Uuid),
    SetPassword(Uuid, String),
    GrantAdmin(Uuid),
    RequestJobRun(JobRunRequest),
    Rerun(RerunRequest),
    RequestLocationWork(LocationWorkRequest),
    LocationWork(Uuid),
    CancelRun {
        event_id: Uuid,
        run_id: Uuid,
        reason: String,
    },
    ChangeTrigger {
        event_id: Uuid,
        trigger_id: Uuid,
        change: TriggerChange,
        reason: String,
    },
}

/// Parsing can finish an invocation before package discovery or any Python handoff.
#[derive(Debug)]
pub(crate) enum NativeParse {
    Exit(i32),
    Invocation {
        namespace: Namespace,
        use_color: bool,
    },
}

/// One turn of named work: the files it wrote, whether metadata should sync, whether it claimed work,
/// and a republish the location now owes a result.
#[derive(Debug, Default)]
pub(crate) struct LocationWorkTurn {
    pub(crate) files: Vec<std::path::PathBuf>,
    pub(crate) sync: bool,
    pub(crate) claimed: bool,
    pub(crate) republish: Option<PendingRepublish>,
}

/// A claimed republish: its result is reported after the next compile, before its lease ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PendingRepublish {
    pub(crate) request_id: uuid::Uuid,
    pub(crate) claim_id: uuid::Uuid,
    pub(crate) deadline: tokio::time::Instant,
}

/// What `sqb skills` installs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkillsOptions {
    pub(crate) global_install: bool,
    pub(crate) targets: Vec<String>,
    pub(crate) force: bool,
}

/// Which `sqb rules` inspection runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RulesAction {
    List,
    Show,
}

/// What `sqb rules list|show` prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RulesOptions {
    pub(crate) action: RulesAction,
    pub(crate) selector: Option<String>,
    pub(crate) json: bool,
}

/// Raw `sqb cost` options; only a pending warehouse refresh needs the Python adapter edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CostOptions {
    pub(crate) selector: String,
    pub(crate) limit: Option<i64>,
    pub(crate) no_limit: bool,
    pub(crate) sort: Option<String>,
    pub(crate) descending: bool,
    pub(crate) since: Option<String>,
    pub(crate) until: Option<String>,
    pub(crate) json: bool,
    pub(crate) json_path: Option<PathBuf>,
    pub(crate) no_color: bool,
}

/// A command that runs without Python.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NativeCommand {
    /// `clean` and a bare `sqb`: observed, with no handler to run.
    Nothing,
    Init,
    Skills(SkillsOptions),
    Rules(RulesOptions),
    Cost(CostOptions),
}

/// One parsed invocation `sqb` serves without Python.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeInvocation {
    pub(crate) command: NativeCommand,
    /// The parsed command name; `None` for a bare `sqb`.
    pub(crate) command_name: Option<String>,
    /// `--project-dir` as given.
    pub(crate) project_dir: Option<PathBuf>,
    pub(crate) target: Option<String>,
    pub(crate) creates_project: bool,
    pub(crate) maintains_skills: bool,
}

/// What a command printed and the status it ends with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CommandOutput {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) exit_code: i32,
}

/// Where the installed `sqlbuild` package keeps what the commands read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledPackage {
    /// The packaged `.agents/skills/sqlbuild` directory.
    pub(crate) skills_root: PathBuf,
}

/// The directories one command runs against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectLocation {
    /// `Path(--project-dir)`, or `Path.cwd()`, unresolved.
    pub(crate) given: PathBuf,
    /// `Path(--project-dir).resolve()`, or `Path.cwd()`.
    pub(crate) resolved: PathBuf,
}

/// A long-running location owns no control-store credentials or executable user objects.
#[derive(Debug)]
pub(crate) struct LocationServe {
    pub(crate) project_dir: PathBuf,
    pub(crate) server_url: String,
    pub(crate) location_id: uuid::Uuid,
    pub(crate) target: String,
    pub(crate) image_id: Option<String>,
    pub(crate) metadata_reconcile_seconds: u32,
    /// Seconds between safety syncs of an idle location; `0` turns them off.
    pub(crate) safety_sync_seconds: u64,
    /// `sqb dev` reads events on stdout and sends control lines on stdin.
    pub(crate) dev_events: bool,
}

/// What `sqb dev` last asked its location: the foreground warehouse verdict and how many recompiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DevControlState {
    /// `None` until `sqb dev` reports its foreground connection; outside `sqb dev`, always connected.
    pub(crate) warehouse: Option<bool>,
    pub(crate) recompiles: u64,
    /// How many of those recompiles also call every discover function again, as `r` and the republish API do.
    pub(crate) rediscovers: u64,
}

/// Retained source observations survive compilation; warehouse exclusions never replace them.
#[derive(Debug)]
pub(crate) struct LocationWatch {
    pub(crate) rules: SnapshotRules,
    pub(crate) local_files: Vec<PathBuf>,
    pub(crate) observed: (Vec<PathStamp>, Option<String>),
    pub(crate) attempted: Option<(Vec<PathStamp>, Option<String>)>,
    pub(crate) changed_at: tokio::time::Instant,
    pub(crate) retry_at: Option<tokio::time::Instant>,
    /// Git commits matter to publication metadata, but asking git every tick spawns a process each time.
    pub(crate) commit_checked_at: tokio::time::Instant,
    /// Whether a generation has published, so the next publication's connection check is not the startup check.
    pub(crate) checked: bool,
    /// A republish just claimed, which the watch loop starts and owes a result.
    pub(crate) republish: Option<PendingRepublish>,
}

/// A bounded supervisor request; dropping the reply cancels and reaps an active compiler.
#[derive(Debug)]
pub(crate) struct LocationCompileRequest {
    pub(crate) generation: i64,
    pub(crate) python_sources: Vec<PathStamp>,
    pub(crate) reply: tokio::sync::oneshot::Sender<Result<LocationLoad, String>>,
}

/// The runtime's reply to a generation: its publication, or the rule findings that failed its compile.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct LocationLoad {
    pub(crate) published: Option<LocationPublication>,
    pub(crate) rules: Option<sqlbuild_orchestration::platform::models::LocationRuleFailure>,
    /// The discovered sets the compile used; empty from a runtime without discover functions.
    #[serde(default)]
    pub(crate) discovered: sqlbuild_orchestration::discovered_sets::types::DiscoveredSets,
    /// Each discover function's file, relative to the project.
    #[serde(default)]
    pub(crate) discover_files: std::collections::BTreeMap<String, String>,
    /// Why nothing was published, redacted; absent after a publication or a rules failure.
    #[serde(default)]
    pub(crate) error: Option<String>,
}

/// A `{"plan": {"argv", "copy"}}` request for the runtime interpreter, answered with its reply.
#[derive(Debug)]
pub(crate) struct LocationPlanRequest {
    pub(crate) request: serde_json::Value,
    pub(crate) reply: tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
}

/// Work for the task that owns the warm runtime interpreter.
#[derive(Debug)]
pub(crate) enum LocationRuntimeRequest {
    Publish(LocationCompileRequest),
    Plan(LocationPlanRequest),
}

/// Only its owner (the compiler task or one metadata synchronization) uses this interpreter's stdio.
#[derive(Debug)]
pub(crate) struct LocationCompiler {
    pub(crate) child: tokio::process::Child,
    pub(crate) stdin: tokio::process::ChildStdin,
    pub(crate) stdout: tokio::io::BufReader<tokio::process::ChildStdout>,
    pub(crate) python_sources: Vec<PathStamp>,
}

/// A delivered source page remains frozen until the API acknowledges it.
#[derive(Debug)]
pub(crate) struct LocationMetadataPage {
    pub(crate) batch: MetadataBatch,
    pub(crate) next_source_after: Option<Vec<Option<String>>>,
}

/// Only local retry and navigation state; the control store keeps no scan progress.
#[derive(Debug)]
pub(crate) struct LocationMetadataSync {
    pub(crate) locations: Vec<WarehouseStateLocation>,
    pub(crate) generation: i64,
    pub(crate) index: usize,
    pub(crate) scan_id: Uuid,
    pub(crate) source_after: Option<Vec<Option<String>>>,
    pub(crate) observed_since: Option<DateTime<Utc>>,
    pub(crate) watermark_loaded: bool,
    pub(crate) statistics: Option<StatisticsRead>,
    pub(crate) reconcile: bool,
    pub(crate) reconcile_at: tokio::time::Instant,
    pub(crate) reconcile_seconds: u32,
    pub(crate) limit: usize,
    /// Survives a failed statistics read's rewind through catalog, which resets `limit`.
    pub(crate) statistics_limit: usize,
    pub(crate) pending: Option<LocationMetadataPage>,
    pub(crate) next_at: tokio::time::Instant,
    /// Between synchronizations; the warehouse is read again only after a run ends.
    pub(crate) idle: bool,
    /// Server time when the current or last synchronization began; `None` before the first.
    pub(crate) cycle_started_at: Option<DateTime<Utc>>,
    /// Server time minus local time, measured as the last synchronization began.
    pub(crate) server_offset: chrono::Duration,
    /// Seconds between safety syncs; `0` turns them off.
    pub(crate) safety_seconds: u64,
    /// When an idle location syncs even though no run ended and nobody asked.
    pub(crate) safety_at: tokio::time::Instant,
    /// Failed reads in a row that did not shrink the page; a delivered page resets it.
    pub(crate) failed_reads: u32,
    /// Failed reads in a row stopped the sync; it rests idle until a run, a request, a publish or a safety sync.
    pub(crate) halted: bool,
    /// Per scan index: the table's change token after its last complete read.
    pub(crate) change_tokens: HashMap<usize, ScanToken>,
    /// The token read before the current scan's first page, recorded once the scan completes.
    pub(crate) scan_token: Option<String>,
    /// Whether any page of the current scan delivered records.
    pub(crate) scan_delivered: bool,
    /// The server already mirrors rows of the current scan's table, per its watermark.
    pub(crate) mirror_holds_rows: bool,
    /// One warm runtime interpreter and warehouse session serve every page of a synchronization.
    pub(crate) reader: Option<LocationCompiler>,
    /// The server's state-history limit, read as each synchronization begins; `None` before the first.
    pub(crate) state_history_versions: Option<u16>,
    /// Progress of the current synchronization, for its per-table lines and summary.
    pub(crate) progress: SyncProgress,
    /// The adapter's history reads start or wake billed compute, as the publication's connection check reports.
    pub(crate) reads_wake_compute: bool,
    /// The server applied a delivered page of the current synchronization, so the synced copy changed.
    pub(crate) applied: bool,
    /// The generation was published after the last plan of what changed, which waits for this sync.
    pub(crate) replan: bool,
}

/// Rows delivered and time taken by the current synchronization and its current table.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SyncProgress {
    pub(crate) sync_started: tokio::time::Instant,
    pub(crate) scan_started: tokio::time::Instant,
    pub(crate) sync_rows: u64,
    pub(crate) scan_rows: u64,
    /// The current table's change token showed it unchanged, so nothing was read.
    pub(crate) scan_unchanged: bool,
    /// Records the server mirrors of the current table, as it counted after the table's last page.
    pub(crate) scan_mirrored: Option<u64>,
}

impl SyncProgress {
    pub(crate) fn new() -> Self {
        let now = tokio::time::Instant::now();
        Self {
            sync_started: now,
            scan_started: now,
            sync_rows: 0,
            scan_rows: 0,
            scan_unchanged: false,
            scan_mirrored: None,
        }
    }
}

/// The metadata reader's answer to one read.
#[derive(Debug)]
pub(crate) enum MetadataReply {
    Page(WarehouseHistoryPage),
    /// Another process holds the warehouse file; nothing was read.
    Busy,
    /// The read failed and delivered nothing; never an empty table.
    Failed(WarehouseHistoryFailure),
}

/// Why a page was not read, with the detail printed to the location's log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReadFailure {
    /// Another process holds the warehouse file; the sync waits, nothing failed.
    Busy,
    /// The metadata reader process did not start.
    ReaderStart(String),
    /// The reader did not connect, the read raised, or the reader broke off.
    Read(WarehouseHistoryFailure),
    /// The reader did not answer within this many seconds.
    TimedOut(u64),
}

impl ReadFailure {
    /// Whether a smaller page may succeed: a time-out, or a read too large for the reader's memory.
    pub(crate) fn page_size(&self) -> bool {
        match self {
            Self::Busy | Self::ReaderStart(_) => false,
            Self::Read(failure) => failure.page_size,
            Self::TimedOut(_) => true,
        }
    }

    /// The closed cause a health report carries; a busy warehouse file is no failure.
    pub(crate) fn cause(&self) -> Option<WarehouseReadFailure> {
        match self {
            Self::Busy => None,
            Self::ReaderStart(_) | Self::Read(_) => Some(WarehouseReadFailure::ReadError {}),
            Self::TimedOut(seconds) => Some(WarehouseReadFailure::TimedOut { seconds: *seconds }),
        }
    }

    /// The failure in words for the location's log and, redacted, the health report.
    pub(crate) fn reason(&self) -> String {
        match self {
            Self::Busy => "another process holds the warehouse file".to_owned(),
            Self::ReaderStart(error) => format!("the metadata reader did not start: {error}"),
            Self::Read(failure) => failure.reason.trim().to_owned(),
            Self::TimedOut(seconds) => format!("no reply within {seconds} s"),
        }
    }
}

/// A table's change token after a complete read, and what that read proves about the mirror.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScanToken {
    pub(crate) token: String,
    /// A complete reconciling read, so the mirror held exactly the table's rows.
    pub(crate) reconciled: bool,
    /// The read delivered rows or read on top of mirrored rows, so an empty mirror lost rows.
    pub(crate) delivered: bool,
    /// The server's newest count of this kind's mirror over every state location, which a skipped read repeats.
    pub(crate) mirrored: Option<u64>,
}

impl LocationMetadataSync {
    /// After a completed sync: whether what changed must be planned again; clears the reasons.
    pub(crate) fn take_replan(&mut self) -> bool {
        let replan = self.replan || self.applied;
        self.replan = false;
        self.applied = false;
        replan
    }

    /// After a plan `take_replan` asked for; one that did not reach the server waits for the next sync.
    pub(crate) fn planned(&mut self, reported: bool) {
        self.replan |= !reported;
    }

    /// A person asked for a sync: start one now when idle, or one more after the current one.
    pub(crate) fn request_sync(&mut self) {
        self.cycle_started_at = None;
        let backing_off = std::mem::take(&mut self.failed_reads) > 0;
        if backing_off || self.idle {
            self.next_at = tokio::time::Instant::now();
        }
    }

    /// Whether an idle location starts its one outstanding sync: first, requested, after a run, or safety.
    pub(crate) fn sync_due(&self, runs_ended_at: Option<DateTime<Utc>>) -> bool {
        let ran = match (self.cycle_started_at, runs_ended_at) {
            (None, _) => true,
            (Some(started_at), Some(ended_at)) => ended_at >= started_at,
            (Some(_), None) => false,
        };
        ran || (self.safety_seconds > 0
            && !self.defers_safety_sync()
            && tokio::time::Instant::now() >= self.safety_at)
    }

    /// Whether an idle location skips interval syncs because history reads wake billed compute.
    pub(crate) fn defers_safety_sync(&self) -> bool {
        self.reads_wake_compute
    }

    /// A scan id timed by the server's clock: a synced plan trusts a record as of its scan's start, never after a read.
    pub(crate) fn server_scan_id(&self) -> Uuid {
        let now: DateTime<Utc> = Utc::now() + self.server_offset;
        let seconds: u64 = u64::try_from(now.timestamp()).unwrap_or_default();
        Uuid::new_v7(Timestamp::from_unix(
            NoContext,
            seconds,
            now.timestamp_subsec_nanos(),
        ))
    }

    /// Start the sync `sync_due` asked for at server time `now`; a stopped sync restarts its pass with a fresh failure count.
    pub(crate) fn begin_sync(&mut self, now: DateTime<Utc>) {
        self.server_offset = now - Utc::now();
        self.scan_id = self.server_scan_id();
        if std::mem::take(&mut self.halted) {
            self.failed_reads = 0;
            self.index = 0;
            self.source_after = None;
            self.observed_since = None;
            self.watermark_loaded = false;
            self.statistics = None;
            self.limit = MAX_HISTORY_PAGE_RECORDS;
            self.scan_token = None;
            self.scan_delivered = false;
        } else {
            self.applied = false;
        }
        self.idle = false;
        self.progress = SyncProgress::new();
        self.cycle_started_at = Some(now);
        self.next_at = tokio::time::Instant::now();
    }

    /// Whether a failed read halves the page: it ran out of time or memory and the page holds more than one.
    pub(crate) fn shrinks(&self, page_size: bool) -> bool {
        let limit: usize = if self.kind() == WarehouseHistoryKind::Statistics {
            self.statistics_limit
        } else {
            self.limit
        };
        page_size && limit > 1
    }

    /// Whether a failed read stops the sync: it does not shrink the page and is the last failure allowed.
    pub(crate) fn halts(&self, page_size: bool) -> bool {
        !self.shrinks(page_size) && self.failed_reads + 1 >= METADATA_FAILURE_LIMIT
    }

    /// Record a failed read and return the wait before the next one; `None` once failures stop the sync.
    pub(crate) fn fail_read(&mut self, page_size: bool) -> Option<std::time::Duration> {
        let shrinks: bool = self.shrinks(page_size);
        self.statistics = None;
        let statistics = self.kind() == WarehouseHistoryKind::Statistics;
        if statistics {
            self.index -= 1;
            self.source_after = None;
            self.scan_id = self.server_scan_id();
            self.observed_since = None;
            self.watermark_loaded = false;
        }
        let limit = if statistics {
            &mut self.statistics_limit
        } else {
            &mut self.limit
        };
        let seconds = if shrinks {
            *limit /= 2;
            METADATA_POLL_SECONDS
        } else {
            self.failed_reads += 1;
            if self.failed_reads >= METADATA_FAILURE_LIMIT {
                self.halted = true;
                self.idle = true;
                self.cycle_started_at = Some(Utc::now() + self.server_offset);
                self.safety_at = tokio::time::Instant::now()
                    + std::time::Duration::from_secs(self.safety_seconds);
                return None;
            }
            METADATA_POLL_SECONDS << (self.failed_reads - 1)
        };
        let wait = std::time::Duration::from_secs(seconds);
        self.next_at = tokio::time::Instant::now() + wait;
        Some(wait)
    }

    /// A synchronization is mid-flight and its next page is due, so the loop need not wait a tick.
    pub(crate) fn paging(&self) -> bool {
        !self.idle && tokio::time::Instant::now() >= self.next_at
    }

    pub(crate) fn reconciling(&self) -> bool {
        self.kind() != WarehouseHistoryKind::Statistics
            && (self.reconcile || self.kind() == WarehouseHistoryKind::Catalog)
    }

    /// Skip a table unchanged since its recorded read, reporting the newest mirrored count of its kind again.
    pub(crate) fn skip_unchanged(&mut self) {
        self.progress.scan_unchanged = true;
        self.progress.scan_mirrored = self
            .change_tokens
            .get(&self.index)
            .and_then(|recorded| recorded.mirrored);
    }

    /// The recorded token a scan may skip on, given whether it reconciles and the mirror holds rows.
    pub(crate) fn unchanged_token(&self) -> Option<&str> {
        let recorded = self.change_tokens.get(&self.index)?;
        ((recorded.reconciled || !self.reconciling())
            && (self.mirror_holds_rows || !recorded.delivered))
            .then_some(recorded.token.as_str())
    }

    /// The newest versions a full read of builds or check results delivers; incremental reads take all.
    pub(crate) fn newest_versions(&self) -> Option<u16> {
        (self.observed_since.is_none()
            && matches!(
                self.kind(),
                WarehouseHistoryKind::Fingerprints | WarehouseHistoryKind::AuditResults
            ))
        .then_some(self.state_history_versions)
        .flatten()
    }

    pub(crate) fn kind(&self) -> WarehouseHistoryKind {
        WarehouseHistoryKind::ALL[self.index % WarehouseHistoryKind::ALL.len()]
    }

    pub(crate) fn location(&self) -> &WarehouseStateLocation {
        &self.locations[self.index / WarehouseHistoryKind::ALL.len()]
    }
}

/// What Python's buffered stdout does with output its reader no longer takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LostStdout {
    /// Kept until exit, whose failed flush Python reports with this exception and status 120.
    AtExit(String),
    /// Dropped by a failed flush; the command's own status stands.
    Dropped,
    /// Raised by the write: an uncaught exception, with this last line and status 1.
    Raised(String),
}

/// What makes two plan requests identical once the generation and synced copy are the same.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlanKey {
    /// The `sqb plan` arguments the location built from the request's settings.
    pub(crate) argv: Vec<String>,
    /// The node names the server froze from the request's selection.
    pub(crate) select: Vec<String>,
    /// Whether the plan read the copy's consumption.
    pub(crate) consumption: bool,
}

/// One finished plan, kept as the JSON text of its document and of the diffs beside it.
#[derive(Debug)]
pub(crate) struct ReusedPlan {
    pub(crate) key: PlanKey,
    pub(crate) document: String,
    pub(crate) diffs: String,
    /// When the plan started, which its windows and expiry follow.
    pub(crate) planned_at: chrono::DateTime<chrono::Utc>,
}

/// The finished plans of one published generation and synced copy, oldest first, and their bytes.
#[derive(Debug)]
pub(crate) struct PlanReuse {
    pub(crate) generation: i64,
    pub(crate) version: String,
    pub(crate) plans: std::collections::VecDeque<std::sync::Arc<ReusedPlan>>,
    pub(crate) bytes: usize,
}

/// A plan the location may keep once its checks pass: its generation, copy, key and start time.
#[derive(Clone, Debug)]
pub(crate) struct PlanToKeep {
    pub(crate) generation: i64,
    pub(crate) version: String,
    pub(crate) key: PlanKey,
    pub(crate) planned_at: chrono::DateTime<chrono::Utc>,
}

/// The copy file the location last read, the mirror version it holds and whether it has consumption.
#[derive(Clone, Debug)]
pub(crate) struct HeldCopy {
    pub(crate) target: String,
    pub(crate) generation: i64,
    pub(crate) version: String,
    pub(crate) consumption: bool,
    pub(crate) path: PathBuf,
}

/// Where the events come from: a server through the saved session, or a local `sqb dev` store.
#[derive(Clone, Debug)]
pub(crate) enum EventSource {
    Server {
        origin: String,
        ca: Option<PathBuf>,
        location_id: uuid::Uuid,
    },
    Development {
        store_url: String,
    },
}

/// One trial: the sensor, the project and target its code runs for, and how the event is chosen.
#[derive(Clone, Debug)]
pub(crate) struct SensorTrialArguments {
    pub(crate) sensor: String,
    pub(crate) project_dir: PathBuf,
    pub(crate) target: String,
    pub(crate) choice: SensorEventChoice,
    pub(crate) source: EventSource,
}
