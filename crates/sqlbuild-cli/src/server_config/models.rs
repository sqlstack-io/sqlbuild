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
    pub(crate) oidc: Option<OidcAuthentication>,
    pub(crate) trusted_proxy: Option<TrustedProxyAuthentication>,
    #[serde(default)]
    pub(crate) launchers: Launchers,
}

/// The listener, control database variable, password sign-in, default role and scheduler.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ServerSection {
    pub(crate) host: Option<IpAddr>,
    pub(crate) port: Option<u16>,
    pub(crate) database_env: Option<String>,
    pub(crate) password_origin: Option<String>,
    pub(crate) default_role: Option<String>,
    pub(crate) scheduler: Option<bool>,
}

/// How long the server keeps statistics, audit records, snapshots and run logs.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetentionSection {
    pub(crate) statistics_days: Option<u16>,
    pub(crate) audit_days: Option<u16>,
    pub(crate) snapshot_count: Option<u16>,
    pub(crate) run_log_days: Option<u16>,
    pub(crate) run_log_count: Option<u16>,
}
