//! Typed HTTP responses shared with the generated OpenAPI contract.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::api::_helpers::authentication::password_sessions;
use crate::api::_helpers::authentication::{cli_login, oidc};
use crate::api::_helpers::resources::{
    administrators, audit, changed_builds, changes, location_health, location_work,
    person_bootstrap, person_checks, person_location_work, person_maintenance, person_node_sql,
    person_operations, person_plans, person_reads, person_selection, run_reads, runs,
    service_accounts, snapshots, trigger_reads, warehouse_metadata,
};

#[derive(Debug, utoipa::OpenApi)]
#[openapi(paths(
    administrators::change,
    administrators::assign,
    administrators::people,
    administrators::holdings,
    administrators::definitions,
    administrators::server,
    service_accounts::list,
    service_accounts::create,
    service_accounts::rotate,
    service_accounts::revoke,
    service_accounts::disable,
    service_accounts::role,
    audit::read,
    person_operations::request_job,
    person_operations::request_adhoc,
    person_operations::check_adhoc,
    person_operations::cancel_run,
    person_operations::rerun,
    person_operations::change_trigger,
    person_operations::recent_runs,
    person_operations::acknowledge,
    person_operations::acknowledgements,
    person_operations::acknowledge_attention,
    person_operations::attention_acknowledgements,
    person_reads::run_nodes,
    person_reads::nodes,
    person_reads::node_states,
    person_reads::node_runs,
    person_reads::run_events,
    person_reads::run_statements,
    person_reads::run_ignored_audits,
    person_reads::run_cost,
    person_reads::cost_summary,
    person_reads::node_costs,
    person_reads::run_statement_sql,
    person_reads::run_output,
    person_reads::run_node_error,
    person_reads::jobs,
    person_reads::job,
    person_reads::providers,
    person_reads::triggers,
    person_reads::ticks,
    person_reads::trigger_changes,
    person_reads::sensor_state,
    person_reads::snapshots,
    person_reads::health,
    person_reads::problems,
    person_reads::platform_events,
    person_operations::catalog,
    person_operations::statistics,
    person_operations::fingerprint,
    person_operations::node_status,
    person_operations::column_lineage,
    person_checks::list,
    person_checks::history,
    person_checks::sql,
    person_checks::evidence,
    person_maintenance::read,
    person_bootstrap::read,
    person_maintenance::coverage,
    person_location_work::request,
    person_location_work::request_sync,
    person_location_work::result,
    person_plans::request,
    person_plans::read,
    person_plans::summary,
    person_plans::rows,
    person_plans::diff,
    person_selection::evaluate,
    person_selection::vocabulary,
    person_node_sql::read,
    password_sessions::login,
    password_sessions::current,
    password_sessions::logout,
    password_sessions::change_password,
    password_sessions::create_person,
    cli_login::start,
    cli_login::approve,
    cli_login::claim,
    oidc::login,
    oidc::callback,
    changes::subscribe,
    changes::subscribe_person,
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
    warehouse_metadata::preview_copy,
    changed_builds::report,
    warehouse_metadata::statistics_due,
    warehouse_metadata::sync_due,
    warehouse_metadata::watermark
))]
pub(crate) struct ControlApi;

/// A page of runs, newest first; the `before_*` pair names the last run of the previous page. `queued` lists the queue instead.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunsQuery {
    pub(crate) limit: Option<u32>,
    #[serde(default)]
    pub(crate) queued: bool,
    pub(crate) before_queued_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(crate) before_run_id: Option<uuid::Uuid>,
}

/// A page of platform events, newest first; `before` is the last storage ID of the previous page.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlatformEventsQuery {
    pub(crate) before: Option<i64>,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadQuery {
    pub(crate) limit: Option<u32>,
}

/// A cost read's window in days (default 7, at most 90) and, for history, its row limit.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CostQuery {
    pub(crate) days: Option<u32>,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunEventsQuery {
    pub(crate) after: Option<i64>,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatementSqlQuery {
    pub(crate) statement_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadAuditQuery {
    pub(crate) after: Option<i64>,
    pub(crate) before: Option<i64>,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunRecallQuery {
    pub(crate) invocation: String,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CheckResultPath {
    pub(crate) location_id: uuid::Uuid,
    pub(crate) target: String,
    pub(crate) binding_key: String,
    pub(crate) event_id: uuid::Uuid,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RunNodePath {
    pub(crate) location_id: uuid::Uuid,
    pub(crate) target: String,
    pub(crate) run_id: uuid::Uuid,
    pub(crate) node_key: String,
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
    /// Existing durable denial audit ID; absent when no decision was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_id: Option<uuid::Uuid>,
}

/// How people sign in to this server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LoginMode {
    Password,
    TrustedProxy,
    Oidc,
    /// `sqb dev` on loopback: no sign-in, every request acts as the local development person.
    Disabled,
}

/// What the UI needs first: how people sign in, and what this person may see and do.
#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
pub struct ControlBootstrap {
    pub login_mode: LoginMode,
    pub access: crate::authorization::models::PersonAccess,
    /// The display name a trusted proxy reported for this request, if it reports one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// The email address a trusted proxy reported for this request, if it reports one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Where signing out goes when a trusted proxy owns the sign-in; absent means no sign-out link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logout_url: Option<String>,
}

/// SSE invalidation topics; `all` means refetch the authorized REST resources.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChangeNotice {
    pub topic: String,
    /// `node_status` only: the state version after this change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    /// `node_status` only: the version this change applies to; refetch node states when it is not yours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_version: Option<u64>,
    /// `node_status` only: the nodes whose state changed, absent when `full` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub states: Option<std::collections::BTreeMap<String, crate::node_states::models::NodeState>>,
    /// `node_status` only: too many nodes changed, or the snapshot or changed build is new; refetch node states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full: Option<bool>,
}

/// Explicit local development scope; never enabled by ordinary server startup.
#[derive(Clone, Debug)]
pub struct DevelopmentScope {
    pub location_id: uuid::Uuid,
    pub target: String,
    pub triggers: bool,
    /// The local person every loopback request acts as.
    pub principal_id: uuid::Uuid,
}

/// The one way people sign in to this server, if any.
#[derive(Debug, Default)]
pub enum HumanLogin {
    #[default]
    None,
    Development(DevelopmentScope),
    Password(String),
    TrustedProxy(TrustedProxyAuthentication),
    Oidc(OidcAuthentication),
}

#[derive(Debug, Default)]
pub struct ApiAuthentication {
    pub login: HumanLogin,
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

/// A proxy names each signed-in person in a header that only `trusted_peers` may supply.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedProxyAuthentication {
    pub origin: String,
    /// The proxy's addresses; both loopback addresses when unset, for a proxy on the same host.
    #[serde(
        default = "crate::api::_helpers::authentication::proxy_settings::default_trusted_peers"
    )]
    pub trusted_peers: std::collections::BTreeSet<std::net::IpAddr>,
    /// Host names a Host-preserving proxy sends, beyond loopback names and the origin's host.
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    /// Usernames pre-bound to existing principals; a binding wins over provisioning.
    #[serde(default)]
    pub identities: std::collections::BTreeMap<String, uuid::Uuid>,
    /// The header naming the signed-in person.
    #[serde(
        default = "crate::api::_helpers::authentication::proxy_settings::default_username_header"
    )]
    pub username_header: String,
    /// A header carrying the person's display name, shown in the UI and never stored.
    #[serde(default)]
    pub display_name_header: Option<String>,
    /// A header carrying the person's email address, shown in the UI and never stored.
    #[serde(default)]
    pub email_header: Option<String>,
    #[serde(default)]
    pub unknown_users: UnknownProxyUsers,
    /// Where the UI's sign-out link goes, since only the proxy can end its own sign-in.
    #[serde(default)]
    pub proxy_logout_url: Option<String>,
}

/// What happens to a proxy username that is neither bound nor provisioned yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownProxyUsers {
    /// Create the person with the installation-wide viewer role; admins grant more in the UI.
    #[default]
    ProvisionViewer,
    /// Refuse the request.
    Reject,
}

/// Who the proxy says this request's person is, beyond the username; read per request, never stored.
#[derive(Clone, Debug, Default)]
pub(crate) struct ProxyProfile {
    pub(crate) display_name: Option<String>,
    pub(crate) email: Option<String>,
}

/// The proxy's sign-out URL, shown to people signed in through it.
#[derive(Clone, Debug)]
pub(crate) struct ProxyLogoutUrl(pub(crate) String);

/// Exact issuer/subject bindings authenticate existing people; beta group mappings grant roles.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcAuthentication {
    pub origin: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret_env: Option<String>,
    pub certificate_authority: Option<std::path::PathBuf>,
    pub identities: std::collections::BTreeMap<String, uuid::Uuid>,
    /// The ID-token claim that lists the person's groups; no claim means no groups.
    #[serde(default)]
    pub groups_claim: Option<String>,
    #[serde(default)]
    pub group_roles: Vec<GroupRole>,
}

/// The verified ID token's claims beyond the standard ones, read only for the groups claim.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct OidcExtraClaims(pub(crate) serde_json::Map<String, serde_json::Value>);

impl openidconnect::AdditionalClaims for OidcExtraClaims {}

/// Members of an identity-provider group get a role, optionally in one location and target.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupRole {
    pub group: String,
    pub role: crate::credentials::models::RoleKey,
    pub location_id: Option<uuid::Uuid>,
    pub target: Option<String>,
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

/// A person to add who signs in with a password: their sign-in name, first password and built-in role everywhere, if any.
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PasswordPersonRequest {
    pub(crate) name: String,
    pub(crate) password: String,
    #[serde(default)]
    pub(crate) role: Option<crate::credentials::models::BuiltinRole>,
}

impl std::fmt::Debug for PasswordPersonRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PasswordPersonRequest")
            .field("name", &self.name)
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

/// The person just added.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonCreated {
    pub principal_id: uuid::Uuid,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct PasswordSessionResponse {
    pub identity: crate::credentials::models::SessionIdentity,
    pub csrf_token: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CliLoginStartRequest {
    pub(crate) client_hash: String,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BrowserLoginMethod {
    Password,
    Oidc,
    Proxy,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CliLoginApprovalRequest {
    pub(crate) browser_code: String,
    pub(crate) approve: bool,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CliLoginClaimRequest {
    pub(crate) flow_id: uuid::Uuid,
    pub(crate) client_secret: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct CliLoginSessionResponse {
    pub(crate) identity: crate::credentials::models::SessionIdentity,
    pub(crate) bearer: String,
}

impl std::fmt::Debug for CliLoginApprovalRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CliLoginApprovalRequest")
            .field("approve", &self.approve)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for CliLoginClaimRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CliLoginClaimRequest")
            .field("flow_id", &self.flow_id)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for CliLoginSessionResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CliLoginSessionResponse")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
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
    pub request_id: uuid::Uuid,
}

#[derive(Clone, Debug)]
pub(crate) struct ChangeFeed {
    pub(crate) sender: tokio::sync::broadcast::Sender<crate::store::models::ChangeMessage>,
    pub(crate) stopped: tokio::sync::watch::Receiver<bool>,
}

/// Who reads a change stream, rechecked before every event; credentials are sensitive header values.
#[derive(Debug)]
pub(crate) enum ChangeSubscriber {
    /// The loopback development scope, which has no credential to recheck.
    Development,
    /// A location token.
    Location(axum::http::HeaderValue),
    /// A person, by browser or CLI session or by trusted proxy; sees no run IDs.
    Person {
        principal_id: uuid::Uuid,
        session: Option<axum::http::HeaderValue>,
        default_role: crate::credentials::models::DefaultRole,
    },
}

/// Sensitive headers redact the bearer from Debug output and all wire notifications.
#[derive(Debug)]
pub(crate) struct ChangeStream {
    pub(crate) store: crate::store::models::ControlStore,
    pub(crate) subscriber: ChangeSubscriber,
    pub(crate) location_id: uuid::Uuid,
    pub(crate) target: String,
    pub(crate) target_hash: String,
    pub(crate) receiver: tokio::sync::broadcast::Receiver<crate::store::models::ChangeMessage>,
    pub(crate) updates:
        tokio::sync::broadcast::Receiver<crate::node_states::models::NodeStatusUpdate>,
    /// Keeps the target's node states refreshed while the stream is open.
    pub(crate) _watch: crate::node_states::models::NodeStatesWatch,
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

/// One file of the built web UI, embedded at compile time and served at `path`.
#[derive(Debug)]
pub(crate) struct UiAsset {
    pub(crate) path: &'static str,
    pub(crate) body: &'static [u8],
}

/// A decompressing reader that fails once it would yield more than `remaining` bytes.
#[derive(Debug)]
pub(crate) struct BoundedReader<R> {
    pub(crate) inner: R,
    pub(crate) remaining: usize,
    pub(crate) exceeded: bool,
}

/// What the server was started with, reported by the settings read.
#[derive(Clone, Debug, Default)]
pub struct ServerFacts {
    pub version: String,
    /// Whether this process runs the scheduler loop.
    pub scheduler: bool,
    pub audit: crate::credentials::models::AuditRetention,
    pub statistics: crate::warehouse_metadata::models::StatisticsRetention,
    pub snapshots: crate::snapshots::models::SnapshotRetention,
    pub run_logs: crate::runs::models::RunLogRetention,
    pub state_history: crate::warehouse_metadata::models::StateHistoryRetention,
    /// Each launcher's `pool_caps`; empty where this server runs no launcher.
    pub pool_caps: crate::declarations::models::ServerPoolCaps,
    /// How long an acknowledged attention condition stays hidden while unchanged.
    pub attention_snooze: crate::acknowledgements::models::AttentionSnooze,
}

/// The server's version, storage, sign-in and retention, for administrators.
#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
pub struct ServerSettings {
    pub version: String,
    pub login_mode: LoginMode,
    /// Whether this server runs the scheduler; another server may hold the lease.
    pub scheduler: bool,
    pub installation: crate::platform::models::InstallationRecord,
    pub retention: ServerRetention,
}

/// How long the server keeps each kind of record.
#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
pub struct ServerRetention {
    pub audit_days: i64,
    pub statistics_days: i64,
    /// Publications kept per location target.
    pub snapshots_kept: i64,
    /// Days a finished run keeps its log of events, statements and output.
    pub run_log_days: i64,
    /// Finished runs per location target that keep their logs.
    pub run_logs_kept: i64,
    /// Builds of each node, and results of each check, the store mirrors from the warehouse.
    pub state_history_versions: i64,
    pub platform_alert_days: i64,
    pub heartbeat_days: i64,
    pub location_health_days: i64,
    pub sensor_evaluation_days: i64,
}

/// When each service key's refusal was last audited, kept per router so the throttle stays local.
#[derive(Clone, Debug, Default)]
pub(crate) struct KeyRefusals(pub(crate) Arc<Mutex<HashMap<uuid::Uuid, Instant>>>);
