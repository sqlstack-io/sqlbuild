//! Live evidence permission on the check's node, one result's rows and the sensitive-read audit share one writer fence.

use uuid::Uuid;

use crate::credentials::models::{Action, AuditContext};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_history::_helpers::checks::{check_result, evidence, sensitive_check_read};
use crate::warehouse_history::models::CheckEvidence;

pub async fn read_person_check_evidence(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    check: (Uuid, &str, &str),
    event_id: Uuid,
) -> Result<Option<CheckEvidence>, ControlError> {
    let (transaction, row) =
        check_result(begin(store, TransactionMode::Write).await?, check, event_id).await?;
    sensitive_check_read(
        (transaction, credentials),
        check,
        (row, Action::ViewEvidence, "evidence.read"),
        evidence,
    )
    .await
}
