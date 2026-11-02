//! Typed HTTP responses shared with the generated OpenAPI contract.

use serde::{Deserialize, Serialize};

use crate::api::_helpers::authentication::oidc;
use crate::api::_helpers::authentication::password_sessions;
use crate::api::_helpers::authentication::trusted_proxy;
use crate::api::_helpers::resources::{
    administrators, audit, changes, location_health, location_work, person_operations, run_reads,
    runs, snapshots, trigger_reads, warehouse_metadata,
};

#[derive(Debug, utoipa::OpenApi)]
#[openapi(paths(
    administrators::change,
    administrators::assign,
    audit::read,
    person_operations::request_job,
    person_operations::cancel_run,
    person_operations::rerun,
    person_operations::change_trigger,
    person_operations::recent_runs,
    password_sessions::login,
    password_sessions::current,
    password_sessions::logout,
    password_sessions::change_password,
    trusted_proxy::identity,
    oidc::login,
    oidc::callback,
    changes::subscribe,
    location_health::report,
    location_health::recent,
    location_work::poll,
    location_work::complete,
    location_work::result,
    snapshots::publish,
    snapshots::current,
    snapshots::metadata,
    runs::execution,
    runs::report,
    runs::heartbeat,
    runs::output,
    runs::recall,
    run_reads::recent,
    run_reads::nodes,
    run_reads::invocations,
    run_reads::recall,
    trigger_reads::current,
    trigger_reads::events,
    trigger_reads::ticks,
    warehouse_metadata::report,
    warehouse_metadata::report_fingerprints,
    warehouse_metadata::watermark
))]
pub(crate) struct ControlApi;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadQuery {
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadAuditQuery {
    pub(crate) after: Option<i64>,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunRecallQuery {
    pub(crate) invocation: String,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RunRecallPath {
    pub(crate) location_id: uuid::Uuid,
    pub(crate) target: String,
    pub(crate) run_id: uuid::Uuid,
    pub(crate) view: String,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ReadResource {
    Run,
    Trigger,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MutationResponse {
    pub applied: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct HeartbeatResponse {
    pub cancel_requested: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ApiErrorBody {
    pub error: String,
}

/// SSE invalidation topics; `all` means refetch the authorized REST resources.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChangeNotice {
    pub topic: String,
}

/// Explicit local development scope; never enabled by ordinary server startup.
#[derive(Clone, Debug)]
pub struct DevelopmentScope {
    pub location_id: uuid::Uuid,
    pub target: String,
    pub triggers: bool,
}

#[derive(Debug, Default)]
pub struct ApiAuthentication {
    pub development_scope: Option<DevelopmentScope>,
    pub password_origin: Option<String>,
    pub trusted_proxy: Option<TrustedProxyAuthentication>,
    pub oidc: Option<OidcAuthentication>,
    pub default_role: crate::credentials::models::DefaultRole,
}

/// Only the server's installation configuration selects the daemon and container to inspect.
#[derive(Clone, Debug)]
pub struct DockerImageSource {
    pub location_id: uuid::Uuid,
    pub target: String,
    pub socket: std::path::PathBuf,
    pub container: String,
}

#[derive(Clone, Debug)]
pub(crate) struct BrowserOrigin(pub(crate) String);

/// Explicit person bindings require an upstream-replaced X-SQB-User; no provisioning or grants.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedProxyAuthentication {
    pub origin: String,
    pub trusted_peers: std::collections::BTreeSet<std::net::IpAddr>,
    pub identities: std::collections::BTreeMap<String, uuid::Uuid>,
}

/// Exact issuer/subject bindings authenticate existing people without provisioning or grants.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcAuthentication {
    pub origin: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret_env: Option<String>,
    pub certificate_authority: Option<std::path::PathBuf>,
    pub identities: std::collections::BTreeMap<String, uuid::Uuid>,
}

#[derive(Debug)]
pub(crate) struct OidcRuntime {
    pub(crate) settings: OidcAuthentication,
    pub(crate) client_secret: Option<openidconnect::ClientSecret>,
    pub(crate) http: reqwest::Client,
    pub(crate) configuration_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OidcCallback {
    pub(crate) state: Option<openidconnect::CsrfToken>,
    pub(crate) code: Option<openidconnect::AuthorizationCode>,
}

#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ExternalIdentityResponse {
    pub principal_id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub csrf_token: String,
}

impl std::fmt::Debug for ExternalIdentityResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExternalIdentityResponse")
            .field("principal_id", &self.principal_id)
            .field("session_id", &self.session_id)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PasswordLoginRequest {
    pub(crate) username: String,
    pub(crate) password: String,
}

impl std::fmt::Debug for PasswordLoginRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PasswordLoginRequest")
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PasswordChangeRequest {
    pub(crate) current_password: String,
    pub(crate) new_password: String,
}

impl std::fmt::Debug for PasswordChangeRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PasswordChangeRequest")
            .finish_non_exhaustive()
    }
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct PasswordSessionResponse {
    pub identity: crate::credentials::models::SessionIdentity,
    pub csrf_token: String,
}

impl std::fmt::Debug for PasswordSessionResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PasswordSessionResponse")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AuthenticationDenial {
    pub error: String,
    pub decision_id: uuid::Uuid,
}

#[derive(Clone, Debug)]
pub(crate) struct ChangeFeed {
    pub(crate) sender: tokio::sync::broadcast::Sender<crate::store::models::ChangeMessage>,
    pub(crate) stopped: tokio::sync::watch::Receiver<bool>,
}

/// Sensitive headers redact the bearer from Debug output and all wire notifications.
#[derive(Debug)]
pub(crate) struct ChangeStream {
    pub(crate) store: crate::store::models::ControlStore,
    pub(crate) authorization: Option<axum::http::HeaderValue>,
    pub(crate) location_id: uuid::Uuid,
    pub(crate) target: String,
    pub(crate) target_hash: String,
    pub(crate) receiver: tokio::sync::broadcast::Receiver<crate::store::models::ChangeMessage>,
    pub(crate) stopped: tokio::sync::watch::Receiver<bool>,
    pub(crate) keepalive: tokio::time::Interval,
    pub(crate) initial: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ControlClient {
    pub(crate) client: reqwest::Client,
    pub(crate) authorization: reqwest::header::HeaderValue,
    pub(crate) base_url: reqwest::Url,
}
