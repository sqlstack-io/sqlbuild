//! Credentials identify a caller; principal permissions are a separate authorization decision.

use std::net::IpAddr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalKind {
    Person,
    ServiceAccount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CredentialScope {
    /// API tokens belong to service accounts, never person sessions.
    Principal {
        principal_id: Uuid,
    },
    Location {
        location_id: Uuid,
    },
    Run {
        run_id: Uuid,
        claim_id: Uuid,
    },
}

/// Server-derived request metadata, never copied from an action's JSON body.
#[derive(Clone, Debug)]
pub struct AuditContext {
    pub actor: String,
    pub granting_role: String,
    pub request_id: Uuid,
    pub ip_address: Option<IpAddr>,
}

/// Closed permission vocabulary; unknown actions fail deserialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
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
    #[serde(rename = "warehouse.sync")]
    WarehouseSync,
    #[serde(rename = "run.adhoc")]
    RunAdhoc,
    #[serde(rename = "connection.test")]
    ConnectionTest,
    #[serde(rename = "plan.preview")]
    PlanPreview,
    #[serde(rename = "location.manage")]
    LocationManage,
    #[serde(rename = "user.manage")]
    UserManage,
    #[serde(rename = "role.assign")]
    RoleAssign,
    #[serde(rename = "token.manage")]
    TokenManage,
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

/// Installation-owned ambient access; never permits an administrative or execution default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultRole {
    #[default]
    None,
    Viewer,
}

/// Server-owned sign-in configuration, used to protect a genuinely usable administrator.
#[derive(Clone, Debug, Default)]
pub enum AdministratorAuthentication {
    #[default]
    Password,
    BoundPeople(std::collections::BTreeSet<Uuid>),
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
    pub role: BuiltinRole,
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
    Denied { decision_id: Uuid, throttled: bool },
}

pub enum AuthenticationAttempt<'a> {
    Login(&'a str),
    Reauthenticate(Uuid),
    IdentityProvider,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ExternalIdentityMechanism {
    TrustedProxy,
    Oidc,
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
