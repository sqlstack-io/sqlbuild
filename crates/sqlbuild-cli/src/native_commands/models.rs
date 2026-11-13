//! Python-free commands, the invocation facts they run with and what they print.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use sqlbuild_cache::project_snapshot::models::{PathStamp, SnapshotRules};
use sqlbuild_orchestration::credentials::models::CredentialScope;
use sqlbuild_orchestration::declarations::models::TriggerChange;
use sqlbuild_orchestration::platform::models::LocationWorkRequest;
use sqlbuild_orchestration::runs::models::{JobRunRequest, RerunRequest};
use sqlbuild_orchestration::warehouse_metadata::models::{
    MetadataBatch, StatisticsRead, WarehouseHistoryKind, WarehouseStateLocation,
};
use uuid::Uuid;

use crate::argv::models::Namespace;
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
}

/// A bounded supervisor request; dropping the reply cancels and reaps an active compiler.
#[derive(Debug)]
pub(crate) struct LocationCompileRequest {
    pub(crate) generation: i64,
    pub(crate) python_sources: Vec<PathStamp>,
    pub(crate) reply: tokio::sync::oneshot::Sender<Result<Option<LocationPublication>, String>>,
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
    /// Seconds between safety syncs; `0` turns them off.
    pub(crate) safety_seconds: u64,
    /// When an idle location syncs even though no run ended and nobody asked.
    pub(crate) safety_at: tokio::time::Instant,
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
}

/// A table's change token after a complete read, and what that read proves about the mirror.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScanToken {
    pub(crate) token: String,
    /// A complete reconciling read, so the mirror held exactly the table's rows.
    pub(crate) reconciled: bool,
    /// The read delivered rows or read on top of mirrored rows, so an empty mirror lost rows.
    pub(crate) delivered: bool,
}

impl LocationMetadataSync {
    /// A person asked for a sync: start one now when idle, or one more after the current one.
    pub(crate) fn request_sync(&mut self) {
        self.cycle_started_at = None;
        if self.idle {
            self.next_at = tokio::time::Instant::now();
        }
    }

    /// Whether an idle location starts a sync: never synced, asked to, a run ended since the last
    /// one began, or the safety interval passed. Each is one outstanding sync, never a second one.
    pub(crate) fn sync_due(&self, runs_ended_at: Option<DateTime<Utc>>) -> bool {
        let ran = match (self.cycle_started_at, runs_ended_at) {
            (None, _) => true,
            (Some(started_at), Some(ended_at)) => ended_at >= started_at,
            (Some(_), None) => false,
        };
        ran || (self.safety_seconds > 0 && tokio::time::Instant::now() >= self.safety_at)
    }

    /// Start the sync `sync_due` asked for, at server time `now`.
    pub(crate) fn begin_sync(&mut self, now: DateTime<Utc>) {
        self.idle = false;
        self.cycle_started_at = Some(now);
        self.next_at = tokio::time::Instant::now();
    }

    /// Failed samples revisit catalog before retrying; no frozen delivery is discarded.
    pub(crate) fn retry_read(&mut self) {
        self.statistics = None;
        if self.kind() == WarehouseHistoryKind::Statistics {
            self.statistics_limit = (self.statistics_limit / 2).max(1);
            self.index -= 1;
            self.source_after = None;
            self.scan_id = Uuid::now_v7();
            self.observed_since = None;
            self.watermark_loaded = false;
        } else {
            self.limit = (self.limit / 2).max(1);
        }
        self.next_at = tokio::time::Instant::now()
            + std::time::Duration::from_secs(super::constants::METADATA_POLL_SECONDS);
    }

    pub(crate) fn reconciling(&self) -> bool {
        self.kind() != WarehouseHistoryKind::Statistics
            && (self.reconcile || self.kind() == WarehouseHistoryKind::Catalog)
    }

    /// The recorded token a scan may skip on, given whether it reconciles and the mirror holds rows.
    pub(crate) fn unchanged_token(&self) -> Option<&str> {
        let recorded = self.change_tokens.get(&self.index)?;
        ((recorded.reconciled || !self.reconciling())
            && (self.mirror_holds_rows || !recorded.delivered))
            .then_some(recorded.token.as_str())
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
