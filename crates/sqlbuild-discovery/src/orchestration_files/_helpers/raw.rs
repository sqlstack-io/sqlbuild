//! The file shapes serde reads before their values are checked.

use serde::Deserialize;
use toml::Spanned;

use crate::orchestration_files::models::{InitialStatus, Overlap};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawSchedule {
    pub(crate) cron: Option<String>,
    pub(crate) calendar: Option<Spanned<String>>,
    pub(crate) timezone: String,
    pub(crate) starts_at: Option<String>,
    #[serde(default)]
    pub(crate) default_status: InitialStatus,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawOnFailure {
    pub(crate) retries: i64,
    #[serde(default)]
    pub(crate) retry_delay_seconds: i64,
}

#[derive(Deserialize)]
pub(crate) struct RawRun {
    pub(crate) name: Spanned<String>,
    pub(crate) description: Spanned<String>,
    #[serde(default)]
    pub(crate) targets: Vec<String>,
    pub(crate) owner: Option<Spanned<String>>,
    #[serde(default)]
    pub(crate) overlap: Overlap,
    #[serde(default)]
    pub(crate) concurrency_groups: Vec<String>,
    pub(crate) schedule: Option<Spanned<RawSchedule>>,
    pub(crate) on_failure: Option<Spanned<RawOnFailure>>,
}

/// The job-only fields; the shared ones are read as `RawRun` from the same text.
#[derive(Deserialize)]
pub(crate) struct RawJob {
    pub(crate) select: Spanned<String>,
    pub(crate) exclude: Option<Spanned<String>>,
    pub(crate) resources: Option<Spanned<toml::Table>>,
    #[serde(default)]
    pub(crate) settings: toml::Table,
}

/// The group-only fields; the shared ones are read as `RawRun` from the same text.
#[derive(Deserialize)]
pub(crate) struct RawGroup {
    pub(crate) jobs: Vec<Spanned<String>>,
}
