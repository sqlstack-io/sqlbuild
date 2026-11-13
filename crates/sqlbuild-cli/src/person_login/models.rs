//! Only public identity is printable; persisted session material is always redacted.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlbuild_orchestration::credentials::models::SessionIdentity;

#[derive(Debug)]
pub(crate) struct LoginOptions {
    pub(crate) action: LoginAction,
    pub(crate) server: String,
    pub(crate) ca: Option<std::path::PathBuf>,
    pub(crate) no_browser: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoginAction {
    Start,
    Status,
    Logout,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PersonCredential {
    pub(super) identity: SessionIdentity,
    pub(super) bearer: String,
}

impl std::fmt::Debug for PersonCredential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PersonCredential")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
pub(super) struct LoginFlow {
    pub(super) browser_code: String,
    pub(super) expires_at: DateTime<Utc>,
}

impl std::fmt::Debug for LoginFlow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LoginFlow")
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub(super) struct PersonClient {
    pub(super) client: reqwest::Client,
    pub(super) origin: String,
}

/// The parts of `GET /api/control/bootstrap` that name the location targets a person can view.
#[derive(Debug, Deserialize)]
pub(super) struct Bootstrap {
    pub(super) access: BootstrapAccess,
}

#[derive(Debug, Deserialize)]
pub(super) struct BootstrapAccess {
    pub(super) locations: Vec<LocationTarget>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LocationTarget {
    pub(super) location_id: uuid::Uuid,
    pub(super) name: String,
    pub(super) target: String,
}
