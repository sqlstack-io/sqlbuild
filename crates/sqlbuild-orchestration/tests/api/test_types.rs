use crate::runs::models::RunStatus;
use crate::store::errors::ControlError;
use crate::store::tests::store_test_types::Backend;

pub(crate) struct ApiContractTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_status: RunStatus,
}

pub(crate) struct DevelopmentTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_status: crate::declarations::models::TriggerStatus,
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

pub(crate) struct CustomRoleApiTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_statuses: [u16; 2],
}
