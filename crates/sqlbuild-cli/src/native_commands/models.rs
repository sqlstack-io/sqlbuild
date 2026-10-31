//! Python-free commands, the invocation facts they run with and what they print.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use sqlbuild_cache::project_snapshot::models::{PathStamp, SnapshotRules};
use sqlbuild_orchestration::credentials::models::{CredentialScope, PrincipalKind};
use sqlbuild_orchestration::declarations::models::TriggerChange;
use sqlbuild_orchestration::platform::models::LocationWorkRequest;
use sqlbuild_orchestration::runs::models::{JobRunRequest, RerunRequest};
use uuid::Uuid;

use crate::argv::models::Namespace;

/// Privileged local operations require direct ownership of the control database.
pub(crate) enum ControlAdmin {
    CreateLocation(String),
    CreatePrincipal(String, PrincipalKind),
    IssueToken(CredentialScope, DateTime<Utc>),
    RevokeToken(Uuid),
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
    pub(crate) reply: tokio::sync::oneshot::Sender<
        Result<Option<sqlbuild_orchestration::snapshots::models::SnapshotRecord>, String>,
    >,
}

/// Only this task owns the isolated interpreter and its private stdio protocol.
#[derive(Debug)]
pub(crate) struct LocationCompiler {
    pub(crate) child: tokio::process::Child,
    pub(crate) stdin: tokio::process::ChildStdin,
    pub(crate) stdout: tokio::io::BufReader<tokio::process::ChildStdout>,
    pub(crate) python_sources: Vec<PathStamp>,
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
