//! Installation launch configuration and the physical children retained until reaping.

use std::collections::BTreeMap;
use std::num::{NonZeroU32, NonZeroUsize};
use std::path::PathBuf;
use std::time::Instant;

use sqlbuild_orchestration::runs::models::RunClaim;
use tokio::process::Child;
use uuid::Uuid;

/// Physical ownership and stop evidence are independent of the scheduler's durable run state.
#[derive(Debug)]
pub(super) enum WorkerChild {
    Local(Child),
    Docker(DockerWorker),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum WorkerExit {
    Code(i64),
    Absent,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Launchers {
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
    pub(super) location_id: Uuid,
    pub(super) target: String,
    pub(super) server_url: String,
    pub(super) max_workers: NonZeroUsize,
    pub(super) socket: PathBuf,
    pub(super) location_container: String,
    pub(super) executable: String,
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
}

/// Physical ownership survives leadership loss; only the current fence changes durable lifecycle.
#[derive(Debug)]
pub(crate) struct Workers {
    pub(super) configuration: Launchers,
    pub(super) active: Vec<ClaimedWorker>,
    pub(super) control_variable: String,
    pub(super) recovered_epoch: Option<i64>,
}
