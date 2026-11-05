//! Credentials identify a caller; principal permissions are a separate authorization decision.

use std::net::IpAddr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::credentials::constants::DEFAULT_AUDIT_RETENTION_DAYS;
use crate::store::errors::ControlError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CredentialScope {
    Location { location_id: Uuid },
    Run { run_id: Uuid, claim_id: Uuid },
}

/// Server-derived request metadata, never copied from an action's JSON body.
#[derive(Clone, Debug)]
pub struct AuditContext {
    pub actor: String,
    pub granting_role: String,
    pub request_id: Uuid,
    pub ip_address: Option<IpAddr>,
}

/// Durable security facts with an exclusive storage-order cursor.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SecurityAuditEvent {
    pub storage_id: i64,
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub actor: String,
    pub action: String,
    pub granting_role: String,
    pub request_id: Uuid,
    pub ip_address: Option<String>,
    pub scope_json: serde_json::Value,
    pub before_json: serde_json::Value,
    pub after_json: serde_json::Value,
}

/// Installation configuration: rows past the horizon stay until every sink and retry lookup is done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuditRetention {
    days: u16,
}

impl AuditRetention {
    pub fn days(days: u16) -> Result<Self, ControlError> {
        if days == 0 {
            return Err(ControlError::Invalid(
                "audit retention must keep at least one day".into(),
            ));
        }
        Ok(Self { days })
    }

    pub(crate) fn cutoff(self, now: DateTime<Utc>) -> DateTime<Utc> {
        now - chrono::Duration::days(i64::from(self.days))
    }
}

impl Default for AuditRetention {
    fn default() -> Self {
        Self {
            days: DEFAULT_AUDIT_RETENTION_DAYS,
        }
    }
}

/// Closed permission vocabulary; unknown actions fail deserialization.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
pub enum Action {
    #[serde(rename = "view")]
    View,
    #[serde(rename = "view.sql")]
    ViewSql,
    #[serde(rename = "view.evidence")]
    ViewEvidence,
    #[serde(rename = "job.run")]
    JobRun,
    #[serde(rename = "run.cancel")]
    RunCancel,
    #[serde(rename = "run.rerun")]
    RunRerun,
    #[serde(rename = "run.rewrite_history")]
    RunRewriteHistory,
    #[serde(rename = "trigger.toggle")]
    TriggerToggle,
    #[serde(rename = "sensor.reset_cursor")]
    SensorResetCursor,
    #[serde(rename = "run.adhoc")]
    RunAdhoc,
    #[serde(rename = "connection.test")]
    ConnectionTest,
    #[serde(rename = "user.manage")]
    UserManage,
    #[serde(rename = "role.assign")]
    RoleAssign,
    #[serde(rename = "audit.view")]
    AuditView,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinRole {
    Viewer,
    Operator,
    Developer,
    Admin,
}

/// A built-in role by name, or a custom role a location publishes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum RoleKey {
    Builtin(BuiltinRole),
    Custom(String),
}

impl From<BuiltinRole> for RoleKey {
    fn from(role: BuiltinRole) -> Self {
        Self::Builtin(role)
    }
}

/// One grant of a project role: node selectors, their permissions, and each selector's nodes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleGrant {
    pub nodes: std::collections::BTreeSet<String>,
    pub permissions: std::collections::BTreeSet<Action>,
    #[serde(default)]
    pub resolved_nodes: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
}

/// One project role of a `roles.toml` file or a published `roles` document.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleDefinition {
    pub description: String,
    pub grants: Vec<RoleGrant>,
}

/// The reviewed project-root `roles.toml` file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RolesFile {
    #[serde(default)]
    pub(crate) roles: std::collections::BTreeMap<String, RoleDefinition>,
}

/// Installation-owned ambient access; never permits an administrative or execution default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultRole {
    #[default]
    None,
    Viewer,
}

/// Scope is computed by the server from the operation, never accepted as a claimed write set.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct PermissionScope {
    pub(crate) action: Action,
    pub(crate) location_id: Option<Uuid>,
    pub(crate) target: Option<String>,
    pub(crate) nodes: std::collections::BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RoleAssignmentRequest {
    pub principal_id: Uuid,
    pub role: RoleKey,
    pub location_id: Option<Uuid>,
    pub target: Option<String>,
    #[serde(default)]
    pub revoke: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenIdentity {
    pub token_id: Uuid,
    pub scope: CredentialScope,
    pub expires_at: DateTime<Utc>,
}

/// Proof of one password version, not authorization or an issued session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedPassword {
    pub principal_id: Uuid,
    pub credential_id: Uuid,
}

#[derive(Clone, Copy, Debug)]
pub enum PasswordAuthority {
    Operator(Uuid),
    Reauthenticated {
        proof: VerifiedPassword,
        session_id: Uuid,
    },
}

/// Administrator membership and person eligibility share the final-admin invariant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AdministratorChange {
    Grant,
    Revoke,
    Disable,
    Enable,
}

/// The person a request authenticated as, and their session unless a trusted proxy vouched.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthenticatedPerson(pub(crate) Uuid, pub(crate) Option<Uuid>);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SessionIdentity {
    pub session_id: Uuid,
    pub principal_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

pub enum SessionAccess<'a> {
    Read,
    Write { csrf: &'a str },
}

pub struct IssuedSession {
    pub identity: SessionIdentity,
    pub bearer: String,
    pub csrf: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct StartedLogin {
    pub(crate) browser_code: String,
    pub(crate) expires_at: DateTime<Utc>,
}

#[derive(Debug)]
pub(crate) enum ClaimedLogin {
    Pending,
    Unavailable,
    Authenticated(IssuedSession),
}

#[derive(Debug, Deserialize)]
pub(crate) struct StoredLogin {
    pub(crate) client_hash: String,
    pub(crate) principal_id: Option<Uuid>,
    pub(crate) session_id: Option<Uuid>,
}

impl std::fmt::Debug for StartedLogin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StartedLogin")
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for IssuedSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IssuedSession")
            .field("identity", &self.identity)
            .field("bearer", &"[redacted]")
            .field("csrf", &"[redacted]")
            .finish()
    }
}

#[derive(Debug)]
pub enum PasswordLogin {
    Authenticated(IssuedSession),
    Denied { throttled: bool },
}

pub enum AuthenticationAttempt<'a> {
    Login(&'a str),
    Reauthenticate(Uuid),
    IdentityProvider,
}

#[derive(Debug)]
pub(crate) struct OidcFlowStart {
    pub(crate) state: openidconnect::CsrfToken,
    pub(crate) browser: openidconnect::CsrfToken,
    pub(crate) nonce: openidconnect::Nonce,
    pub(crate) pkce_verifier: openidconnect::PkceCodeVerifier,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StoredOidcFlow {
    pub(crate) flow_id: Uuid,
    pub(crate) browser_hash: String,
    pub(crate) nonce: openidconnect::Nonce,
    pub(crate) pkce_verifier: openidconnect::PkceCodeVerifier,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StoredSession {
    #[serde(flatten)]
    pub(crate) identity: SessionIdentity,
    pub(crate) secret_hash: String,
    pub(crate) csrf_hash: String,
}

/// The bearer is returned once. Debug output must never contain it.
#[derive(Serialize)]
pub struct IssuedToken {
    pub identity: TokenIdentity,
    pub bearer: String,
}

impl std::fmt::Debug for IssuedToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IssuedToken")
            .field("identity", &self.identity)
            .field("bearer", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct StoredToken {
    pub(crate) token_id: Uuid,
    pub(crate) scope_json: CredentialScope,
    pub(crate) secret_hash: String,
    pub(crate) expires_at: DateTime<Utc>,
}
