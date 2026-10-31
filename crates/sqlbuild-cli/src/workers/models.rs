//! Installation launch configuration and the physical children retained until reaping.

use std::collections::BTreeMap;
use std::num::{NonZeroU32, NonZeroUsize};
use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Instant;

use sqlbuild_orchestration::runs::models::RunClaim;
use tokio::process::Child;
use uuid::Uuid;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LocalLaunchers {
    pub(super) local: Vec<LocalLauncher>,
    #[serde(default)]
    pub(super) concurrency_limits: BTreeMap<String, NonZeroU32>,
}

/// No executable, project path or API address is supplied by an untrusted run request.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LocalLauncher {
    pub(super) location_id: Uuid,
    pub(super) target: String,
    pub(super) project_dir: PathBuf,
    pub(super) executable: PathBuf,
    pub(super) server_url: String,
    pub(super) max_workers: NonZeroUsize,
}

#[derive(Debug)]
pub(super) struct LocalWorker {
    pub(super) claim: RunClaim,
    pub(super) child: Option<Child>,
    pub(super) stopping: Option<Instant>,
    pub(super) exit: Option<ExitStatus>,
}

/// Physical ownership survives leadership loss; only the current fence changes durable lifecycle.
#[derive(Debug)]
pub(crate) struct LocalWorkers {
    pub(super) configuration: LocalLaunchers,
    pub(super) active: Vec<LocalWorker>,
    pub(super) control_variable: String,
}
