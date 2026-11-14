//! Installation launch configuration and the physical children retained until reaping.

use std::collections::BTreeMap;
use std::num::{NonZeroU32, NonZeroUsize};
use std::path::PathBuf;
use std::time::Instant;

use sqlbuild_orchestration::declarations::models::MemorySize;
use sqlbuild_orchestration::runs::models::RunClaim;
use tokio::process::Child;
use uuid::Uuid;

/// Physical ownership and stop evidence are independent of the scheduler's durable run state.
#[derive(Debug)]
pub(super) enum WorkerChild {
    Local(Child),
    RecoveredLocal(sqlbuild_process_ownership::ownership::models::RecoveredOwnership),
    Docker(DockerWorker),
}

/// A local worker command with the claim ownership it must hold before running any code.
#[derive(Debug)]
pub(super) struct LocalSpawn {
    pub(super) command: tokio::process::Command,
    pub(super) ownership: sqlbuild_process_ownership::ownership::models::PendingOwnership,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum WorkerExit {
    Code(i64),
    Absent,
}

/// The `[launchers]` section of `server.toml`.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Launchers {
    #[serde(default)]
    pub(super) local: Vec<LocalLauncher>,
    #[serde(default)]
    pub(super) docker: Vec<DockerLauncher>,
    #[serde(default)]
    pub(super) concurrency_limits: BTreeMap<String, NonZeroU32>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LauncherScope<'a> {
    pub(super) location_id: Uuid,
    pub(super) target: &'a str,
    pub(super) server_url: &'a str,
    pub(super) max_workers: NonZeroUsize,
}

/// No executable, project path or API address is supplied by an untrusted run request.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LocalLauncher {
    /// The location's name; start-up resolves it to `location_id`.
    pub(super) location: String,
    #[serde(skip)]
    pub(super) location_id: Uuid,
    pub(super) target: String,
    pub(super) server_url: String,
    pub(super) max_workers: NonZeroUsize,
    pub(super) project_dir: PathBuf,
    pub(super) executable: PathBuf,
}

/// The installation chooses the daemon and location container; the snapshot pins the worker image.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DockerLauncher {
    /// The location's name; start-up resolves it to `location_id`.
    pub(super) location: String,
    #[serde(skip)]
    pub(super) location_id: Uuid,
    pub(super) target: String,
    pub(super) server_url: String,
    pub(super) max_workers: NonZeroUsize,
    pub(super) socket: PathBuf,
    pub(super) location_container: String,
    pub(super) executable: String,
    #[serde(default)]
    pub(super) run: RunDefaults,
}

/// Worker container limits for every run of a launcher; a job's `resources` override them up to `max_memory`.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunDefaults {
    pub(super) memory: Option<MemorySize>,
    pub(super) cpus: Option<f64>,
    pub(super) pids: Option<i64>,
    pub(super) cgroup_parent: Option<String>,
    pub(super) max_memory: Option<MemorySize>,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Launcher<'a> {
    Local(&'a LocalLauncher),
    Docker(&'a DockerLauncher),
}

/// Retain the deterministic identity before create/start, including uncertain HTTP responses.
#[derive(Debug)]
pub(super) struct DockerWorker {
    pub(super) client: reqwest::Client,
    pub(super) name: String,
    pub(super) claim_id: Uuid,
    pub(super) image_id: String,
}

#[derive(Debug)]
pub(super) struct ClaimedWorker {
    pub(super) claim: RunClaim,
    pub(super) child: Option<WorkerChild>,
    pub(super) stopping: Option<Instant>,
    pub(super) exit: Option<WorkerExit>,
    pub(super) launch_failure: Option<&'static str>,
    /// Supervision resumed after a scheduler restart rather than from this scheduler's launch.
    pub(super) recovered: bool,
}

/// Physical ownership survives leadership loss; only the current fence changes durable lifecycle.
#[derive(Debug)]
pub(crate) struct Workers {
    pub(super) configuration: Launchers,
    pub(super) active: Vec<ClaimedWorker>,
    pub(super) control_variable: String,
    pub(super) recovered_epoch: Option<i64>,
}
