//! Each section of `server.toml`; a flag given on the command line overrides the file.

use std::net::IpAddr;

use sqlbuild_orchestration::api::models::{OidcAuthentication, TrustedProxyAuthentication};

use crate::workers::models::Launchers;

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ServerConfig {
    #[serde(default)]
    pub(crate) server: ServerSection,
    #[serde(default)]
    pub(crate) retention: RetentionSection,
    #[serde(default)]
    pub(crate) output: OutputSection,
    pub(crate) oidc: Option<OidcAuthentication>,
    pub(crate) trusted_proxy: Option<TrustedProxyAuthentication>,
    #[serde(default)]
    pub(crate) launchers: Launchers,
}

/// The listener, control database variable, password sign-in, default role, scheduler and attention snooze.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ServerSection {
    pub(crate) host: Option<IpAddr>,
    pub(crate) port: Option<u16>,
    pub(crate) database_env: Option<String>,
    pub(crate) password_origin: Option<String>,
    pub(crate) default_role: Option<String>,
    pub(crate) scheduler: Option<bool>,
    pub(crate) attention_snooze: Option<String>,
}

/// How long the server keeps statistics, audit records, snapshots, run logs and state history.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetentionSection {
    pub(crate) statistics_days: Option<u16>,
    pub(crate) audit_days: Option<u16>,
    pub(crate) snapshot_count: Option<u16>,
    pub(crate) run_log_days: Option<u16>,
    pub(crate) run_log_count: Option<u16>,
    pub(crate) state_history_versions: Option<u16>,
    /// Days a finished run's output stays in the output store; defaults to `run_log_days`.
    pub(crate) run_output_days: Option<u16>,
}

/// Where run output is kept and how much of one run is kept.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OutputSection {
    /// `file:///path` or `s3://bucket/prefix`; by default `output` beside a SQLite control store.
    pub(crate) store: Option<String>,
    /// Bytes, as a number or text such as `"20GB"`; past it a run keeps its first and last parts.
    pub(crate) max_bytes_per_run: Option<ByteCount>,
}

/// A byte count written as a number or as text with a unit, such as `"500MB"` or `"2GiB"`.
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub(crate) enum ByteCount {
    Number(u64),
    Text(String),
}
