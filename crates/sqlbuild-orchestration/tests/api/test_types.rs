use sqlbuild_orchestration::runs::models::RunStatus;
use sqlbuild_orchestration::store::errors::ControlError;

use crate::store_test_types::Backend;

pub(crate) struct ApiContractTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_status: RunStatus,
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
