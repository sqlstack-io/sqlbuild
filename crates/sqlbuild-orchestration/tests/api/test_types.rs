use crate::runs::models::RunStatus;
use crate::store::errors::ControlError;
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::models::WarehouseHistoryKind;

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
