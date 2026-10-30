use sqlbuild_orchestration::runs::models::RunStatus;
use sqlbuild_orchestration::store::errors::ControlError;

use crate::store_test_types::Backend;

pub(crate) struct ApiContractTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_status: RunStatus,
}

pub(crate) struct TestApi {
    pub(crate) url: String,
    pub(crate) shutdown: tokio::sync::oneshot::Sender<()>,
    pub(crate) task: tokio::task::JoinHandle<Result<(), ControlError>>,
}
