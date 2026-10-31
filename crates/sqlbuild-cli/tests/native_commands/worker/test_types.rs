use crate::test_types::TestControl;

pub(crate) struct WorkerHeartbeatTestCase {
    pub(crate) description: &'static str,
    pub(crate) interruption: fn(WorkerControl, &mut TestControl),
    pub(crate) expected_cancelled: bool,
}

pub(crate) struct WorkerControl {
    pub(crate) store: sqlbuild_orchestration::store::models::ControlStore,
    pub(crate) context: sqlbuild_orchestration::credentials::models::AuditContext,
    pub(crate) token_id: uuid::Uuid,
    pub(crate) identity: (uuid::Uuid, uuid::Uuid),
}
