//! A republish's state as its requester or another person allowed to republish reads it.

use std::time::Duration;

use chrono::Utc;
use uuid::Uuid;

use crate::authorization::main::decisions::authorize_person::authorize_person;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::platform::_helpers::location::republish_status::republish_status;
use crate::platform::constants::{MAX_REPUBLISH_WAIT_SECONDS, REPUBLISH_POLL_MILLISECONDS};
use crate::platform::main::work::_read_target_work::read_target_work;
use crate::platform::models::{RepublishState, RepublishStatus};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// The target's republish, read again until it finishes or `wait` (capped) seconds pass.
pub async fn read_republish(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Option<Uuid>),
    (location_id, target, request_id): (Uuid, &str, Uuid),
    wait: u64,
) -> Result<Option<RepublishStatus>, ControlError> {
    if let Some(principal_id) = credentials {
        let (transaction, _) = authorize_person(
            begin(store, TransactionMode::Write).await?,
            (context, principal_id),
            &PermissionScope::action(Action::LocationRepublish, Some((location_id, target))),
            DefaultRole::None,
        )
        .await?;
        commit(transaction).await?;
    }
    let deadline =
        tokio::time::Instant::now() + Duration::from_secs(wait.min(MAX_REPUBLISH_WAIT_SECONDS));
    loop {
        let transaction = begin(store, TransactionMode::Read).await?;
        let (transaction, record) =
            read_target_work(transaction, (location_id, target, request_id)).await?;
        commit(transaction).await?;
        let status: Option<RepublishStatus> =
            record.and_then(|record| republish_status(&record, Utc::now()));
        let pending: bool = status
            .as_ref()
            .is_some_and(|status| status.state == RepublishState::Pending);
        let next = tokio::time::Instant::now() + Duration::from_millis(REPUBLISH_POLL_MILLISECONDS);
        if !pending || next > deadline {
            return Ok(status);
        }
        tokio::time::sleep_until(next).await;
    }
}
