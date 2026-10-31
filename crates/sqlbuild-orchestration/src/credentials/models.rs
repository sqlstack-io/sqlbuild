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
    Principal { principal_id: Uuid },
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenIdentity {
    pub token_id: Uuid,
    pub scope: CredentialScope,
    pub expires_at: DateTime<Utc>,
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
