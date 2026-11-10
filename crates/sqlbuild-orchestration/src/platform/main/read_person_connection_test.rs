//! Connection-test results require live action authority and never expose sensor work.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::platform::_helpers::connection_tests::authorize_connection_test;
use crate::platform::main::_read_target_work::read_target_work;
use crate::platform::models::{LocationWork, LocationWorkRecord};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn read_person_connection_test(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target, request_id): (Uuid, &str, Uuid),
) -> Result<Option<LocationWorkRecord>, ControlError> {
    let (transaction, _) = authorize_connection_test(
        begin(store, TransactionMode::Write).await?,
        (context, Some(credentials)),
        (location_id, target),
    )
    .await?;
    let (transaction, record) =
        read_target_work(transaction, (location_id, target, request_id)).await?;
    let result = record
        .filter(|record| matches!(record.request_json.work, LocationWork::CheckConnection {}));
    commit(transaction).await?;
    Ok(result)
}
