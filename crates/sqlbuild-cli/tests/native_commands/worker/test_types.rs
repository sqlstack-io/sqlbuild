use crate::test_types::TestControl;

pub(crate) struct WorkerHeartbeatTestCase {
    pub(crate) description: &'static str,
    pub(crate) interruption: fn(WorkerControl, &mut TestControl),
    pub(crate) expected_cancelled: bool,
}

pub(crate) struct WorkerRecoveryTestCase {
    pub(crate) description: &'static str,
    pub(crate) image_id: Option<&'static str>,
    pub(crate) expected_status: &'static str,
}

pub(crate) struct WorkerControl {
    pub(crate) store: sqlbuild_orchestration::store::models::ControlStore,
    pub(crate) context: sqlbuild_orchestration::credentials::models::AuditContext,
    pub(crate) lease: sqlbuild_orchestration::scheduler::models::SchedulerLease,
    pub(crate) token_id: uuid::Uuid,
    pub(crate) identity: (uuid::Uuid, uuid::Uuid),
}
