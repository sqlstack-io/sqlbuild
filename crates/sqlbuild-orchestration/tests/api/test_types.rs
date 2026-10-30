use sqlbuild_orchestration::store::errors::ControlError;

pub(crate) struct TestApi {
    pub(crate) url: String,
    pub(crate) shutdown: tokio::sync::oneshot::Sender<()>,
    pub(crate) task: tokio::task::JoinHandle<Result<(), ControlError>>,
}
