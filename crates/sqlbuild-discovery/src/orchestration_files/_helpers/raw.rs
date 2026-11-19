//! The file shapes serde reads before their values are checked.

use std::collections::BTreeMap;

use serde::Deserialize;
use toml::Spanned;

use crate::orchestration_files::models::{InitialStatus, Overlap};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawSchedule {
    pub(crate) cron: Option<Spanned<String>>,
    pub(crate) calendar: Option<Spanned<String>>,
    pub(crate) timezone: Spanned<String>,
    pub(crate) starts_at: Option<Spanned<String>>,
    #[serde(default)]
    pub(crate) default_status: InitialStatus,
}

/// `[retry]`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawRetry {
    pub(crate) attempts: Option<Spanned<i64>>,
    pub(crate) delay_seconds: Option<Spanned<i64>>,
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
    pub(crate) pools: BTreeMap<String, Spanned<i64>>,
    pub(crate) schedule: Option<Spanned<RawSchedule>>,
    pub(crate) retry: Option<Spanned<RawRetry>>,
}

/// The job-only fields; the shared ones are read as `RawRun` from the same text.
#[derive(Deserialize)]
pub(crate) struct RawJob {
    pub(crate) select: Spanned<String>,
    pub(crate) exclude: Option<Spanned<String>>,
    pub(crate) resources: Option<Spanned<toml::Table>>,
    #[serde(default)]
    pub(crate) settings: BTreeMap<String, Spanned<toml::Value>>,
}

/// The group-only fields; the shared ones are read as `RawRun` from the same text.
#[derive(Deserialize)]
pub(crate) struct RawGroup {
    pub(crate) jobs: Vec<Spanned<String>>,
}
