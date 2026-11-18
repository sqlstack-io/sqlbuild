use crate::runs::models::RunStatus;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::models::{
    MetadataBatch, WarehouseHistoryKind, WarehouseSyncResponse,
};
use chrono::{DateTime, Utc};
use futures_util::future::BoxFuture;
use uuid::Uuid;

pub(crate) struct ApiContractTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_status: RunStatus,
}

pub(crate) struct PublishEncodingTestCase {
    pub(crate) description: &'static str,
    pub(crate) forged_bearer: Option<&'static str>,
    pub(crate) encoding: &'static str,
    pub(crate) body: fn() -> Vec<u8>,
    pub(crate) expected_status: reqwest::StatusCode,
}

pub(crate) struct OlderServerTestCase {
    pub(crate) description: &'static str,
    pub(crate) refusal: reqwest::StatusCode,
    pub(crate) expected_requests: usize,
}

pub(crate) struct DevelopmentTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_status: crate::declarations::models::TriggerStatus,
    /// The names of the first target's location and of a second target's location.
    pub(crate) expected_names: [&'static str; 2],
}

pub(crate) struct OlderServerReportTestCase {
    pub(crate) description: &'static str,
    pub(crate) error_message: Option<&'static str>,
    pub(crate) expected_upgrade_refusal: bool,
}

pub(crate) struct WorkerRedirectTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_status: reqwest::StatusCode,
}

pub(crate) struct WorkerDeadlineTestCase {
    pub(crate) description: &'static str,
    pub(crate) budget: std::time::Duration,
    pub(crate) expected_kind: sqlbuild_recall::store::errors::StoreErrorKind,
    pub(crate) expected_arrival: bool,
}

pub(crate) struct ApiReadTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_runs: usize,
    pub(crate) expected_ticks: usize,
}

pub(crate) struct TestApi {
    pub(crate) url: String,
    pub(crate) shutdown: tokio::sync::oneshot::Sender<()>,
    pub(crate) task: tokio::task::JoinHandle<Result<(), ControlError>>,
}

pub(crate) struct SseConnection {
    pub(crate) response: reqwest::Response,
    pub(crate) pending: Vec<u8>,
}

pub(crate) struct ResetProxy {
    pub(crate) store: crate::store::models::ControlStore,
    pub(crate) reset: tokio::sync::watch::Sender<u64>,
    pub(crate) shutdown: tokio::sync::oneshot::Sender<()>,
    pub(crate) task: tokio::task::JoinHandle<()>,
}

pub(crate) struct PersonLiveTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_denied_feed: reqwest::StatusCode,
    pub(crate) expected_disabled_bootstrap: reqwest::StatusCode,
}

pub(crate) struct ChangesTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_generation: i64,
}

pub(crate) struct LocationHealthTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_observations: usize,
}

pub(crate) struct LocationWorkTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_generation: i64,
}

pub(crate) struct ColumnLineageTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_orders_edges: Vec<(&'static str, &'static str)>,
    pub(crate) expected_leaf_edges: usize,
}

pub(crate) struct StateTableClassificationTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_tables: Vec<&'static str>,
    pub(crate) expected_last_kinds: [WarehouseHistoryKind; 2],
}

pub(crate) struct WarehouseMetadataTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_observations: usize,
}

pub(crate) struct MirrorConflictTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) kind: WarehouseHistoryKind,
    pub(crate) fixture: &'static str,
    /// The record's source id, time and value fields.
    pub(crate) fields: [&'static str; 3],
    pub(crate) ingest: MirrorIngest,
    /// Two rows of one source id as (time, value), each delivered in its own page.
    pub(crate) rows: [(&'static str, &'static str); 2],
    pub(crate) expected_row: (&'static str, &'static str),
    /// The kept value in its typed mirror column, for kinds that have one.
    pub(crate) expected_column: Option<&'static str>,
}

/// A kept mirror row as (source id, record time, record value, column time, typed column value).
pub(crate) type KeptMirrorRow = (String, DateTime<Utc>, String, DateTime<Utc>, Option<String>);

/// Mirrors one page of a history kind.
pub(crate) type MirrorIngest =
    for<'a> fn(
        &'a ControlStore,
        (Uuid, &'a str),
        &'a MetadataBatch,
    ) -> BoxFuture<'a, Result<WarehouseSyncResponse, ControlError>>;

pub(crate) struct PasswordApiTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_lifetime: i64,
}

pub(crate) struct ProxyApiTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) trusted_peer: &'static str,
    pub(crate) expected_sessions: i64,
}

pub(crate) struct ProxyProvisioningTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// Untrusted peer, missing username, another host, a taken name, and a newcomer under `reject`.
    pub(crate) expected_refusals: [reqwest::StatusCode; 5],
    /// A read under another host name the proxy serves.
    pub(crate) expected_other_host_read: reqwest::StatusCode,
    /// Cross-site post, HTTPS twin, no proof, the origin, then the CSRF header under another host.
    pub(crate) expected_write_statuses: [reqwest::StatusCode; 6],
    /// The newcomer's role assignments as JSON rows.
    pub(crate) expected_roles: &'static str,
    /// The change feed's cache-control, buffering and content-type headers.
    pub(crate) expected_feed_headers: [&'static str; 3],
}

/// One identity-header request from one socket peer to a server signing people in one way.
pub(crate) struct ProxyPeerTrustTestCase {
    pub(crate) description: &'static str,
    pub(crate) login: crate::api::models::HumanLogin,
    pub(crate) peer: &'static str,
    pub(crate) forwarding_headers: &'static [(&'static str, &'static str)],
    pub(crate) username: &'static str,
    pub(crate) expected_status: reqwest::StatusCode,
    pub(crate) expected_provisioned: bool,
}

/// One request from a trusted proxy peer, naming a person, under one `Host` and set of headers.
pub(crate) struct ProxyHostTestCase {
    pub(crate) description: &'static str,
    pub(crate) allowed_hosts: &'static [&'static str],
    pub(crate) peer: &'static str,
    pub(crate) request: (&'static str, &'static str),
    pub(crate) headers: &'static [(&'static str, &'static str)],
    pub(crate) username: &'static str,
    pub(crate) expected_status: reqwest::StatusCode,
    pub(crate) expected_provisioned: bool,
}

pub(crate) struct ProxyPeersDefaultTestCase {
    pub(crate) description: &'static str,
    pub(crate) configuration: serde_json::Value,
    pub(crate) expected_peers: &'static [&'static str],
}

pub(crate) struct ExternalSessionTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_viewer_statuses: [u16; 3],
}

pub(crate) struct OidcApiTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_authentications: usize,
}

pub(crate) struct OidcGroupsTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_granting_role: &'static str,
    pub(crate) expected_rejected_mappings: usize,
}

pub(crate) struct OidcThrottleTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_allowed: usize,
}

pub(crate) struct CliLoginTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_exchanges: usize,
}

/// A refused request's URL and, for writes, its JSON body.
pub(crate) type DenialRequest = (String, Option<serde_json::Value>);

pub(crate) struct DenialTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_status: u16,
}

pub(crate) struct RunIdRefusalTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_refusal_status: u16,
    pub(crate) expected_random_status: u16,
}

pub(crate) struct CustomRoleApiTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_statuses: [u16; 2],
}

pub(crate) struct UiAssetTestCase {
    pub(crate) description: &'static str,
    pub(crate) method: reqwest::Method,
    pub(crate) path: &'static str,
    pub(crate) expected_status: u16,
    pub(crate) expected_body: &'static str,
    pub(crate) expected_cache: Option<&'static str>,
}

pub(crate) struct UiPathTestCase {
    pub(crate) description: &'static str,
    pub(crate) path: &'static str,
    pub(crate) expected_status: u16,
    pub(crate) expected_body: &'static str,
}

pub(crate) struct CompressionTestCase {
    pub(crate) description: &'static str,
    pub(crate) method: reqwest::Method,
    /// `{location}` stands for the fixture's location ID.
    pub(crate) path: &'static str,
    pub(crate) accept_encoding: &'static str,
    pub(crate) expected_status: u16,
    pub(crate) expected_gzip: bool,
}

pub(crate) struct ServerErrorTestCase {
    pub(crate) description: &'static str,
    pub(crate) path: &'static str,
    pub(crate) expected_status: u16,
    pub(crate) expected_body: &'static str,
}

/// The node report shape a server had before failed nodes carried error text.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct OlderNodeReport {
    event_id: uuid::Uuid,
    run_id: uuid::Uuid,
    claim_id: Option<uuid::Uuid>,
    change: OlderNodeFinished,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct OlderNodeFinished {
    kind: String,
    node_key: String,
    outcome: crate::runs::models::NodeOutcome,
    rows_affected: Option<i64>,
    error_code: Option<String>,
    skipped_because_of: Option<String>,
}

pub(crate) struct PlanTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_selection: &'static [&'static str],
    pub(crate) expected_warning: &'static str,
}

pub(crate) struct UpcomingTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_hourly: [&'static str; 5],
    pub(crate) expected_calendar: [&'static str; 3],
}

pub(crate) struct AcknowledgementTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// Statuses of: acknowledge, the same request retried, a run of another target, withdraw.
    pub(crate) expected_statuses: [u16; 4],
    /// Whether acknowledging, retrying and withdrawing each changed anything.
    pub(crate) expected_applied: [bool; 3],
    pub(crate) expected_acknowledged_by: &'static str,
}

pub(crate) struct ProvidersTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_jobs: Vec<&'static str>,
}

pub(crate) struct NodeStatesTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_listed: usize,
}

pub(crate) struct ChangedBuildTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_before_plan: serde_json::Value,
}

pub(crate) struct HeldTargetTestCase {
    pub(crate) description: &'static str,
    pub(crate) watched: bool,
    /// A refresh is due and one is scheduled, as when signals arrived while the target was watched.
    pub(crate) pending: (bool, bool),
    /// Whether the target is still held after the signal, and whether the signal refreshed it.
    pub(crate) expected_held_and_refreshed: (bool, bool),
}

pub(crate) struct PushedStatesTestCase {
    pub(crate) description: &'static str,
    pub(crate) built: usize,
    pub(crate) expected_pushed: Option<usize>,
}

pub(crate) struct PoolCapHealthTestCase {
    pub(crate) description: &'static str,
    pub(crate) caps: &'static [(&'static str, u32)],
    /// Each warning as `(job_key, pool, amount, cap, message)`; empty when no pool_capped observation is expected.
    pub(crate) expected_warnings: &'static [(&'static str, &'static str, u32, u32, &'static str)],
}

pub(crate) struct StateHistoryRetentionTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) configured: Option<u16>,
    pub(crate) expected_versions: u16,
}

/// A SQLite store in one auto-vacuum mode whose pruning frees most of its pages.
pub(crate) struct SqliteReclaimTestCase {
    pub(crate) description: &'static str,
    pub(crate) auto_vacuum: &'static str,
    pub(crate) expected_mode: i64,
}

pub(crate) struct PlanDiffTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The diff of a model the document left out, as it reads back.
    pub(crate) expected_detached_diff: &'static [&'static str],
}

pub(crate) struct IngestionTurnTestCase {
    pub(crate) description: &'static str,
    pub(crate) pages: usize,
    pub(crate) relations_per_page: usize,
    pub(crate) expected_writer_first: bool,
}
