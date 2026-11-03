//! Connection-test results require live action authority and never expose sensor work.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::platform::_helpers::connection_tests::authorize_connection_test;
use crate::platform::main::read_location_work::read_location_work;
use crate::platform::models::{LocationWork, LocationWorkRecord};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn read_person_connection_test(
    store: &ControlStore,
    (context, credentials): (&AuditContext, (&str, &str)),
    (location_id, target, request_id): (Uuid, &str, Uuid),
) -> Result<Option<LocationWorkRecord>, ControlError> {
    let (transaction, _) = authorize_connection_test(
        begin(store, TransactionMode::Write).await?,
        (context, Some(credentials)),
        (location_id, target),
    )
    .await?;
    let (transaction, result) = read_location_work(transaction, request_id).await?;
    let result = result.map(|stored| stored.record).filter(|record| {
        record.request_json.location_id == location_id
            && record.request_json.target == target
            && matches!(record.request_json.work, LocationWork::CheckConnection {})
    });
    commit(transaction).await?;
    Ok(result)
}
