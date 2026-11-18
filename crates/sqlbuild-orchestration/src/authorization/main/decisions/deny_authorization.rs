//! A denial is returned only after its decision ID and authoritative scope commit durably.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlTransaction;

pub(crate) async fn deny<T>(
    transaction: ControlTransaction,
    context: &AuditContext,
    scope: Value,
    reason: &str,
) -> Result<T, ControlError> {
    let decision_id = Uuid::now_v7();
    commit(
        append_audit(
            transaction,
            context,
            (decision_id, "authorization.denied"),
            (
                scope,
                json!(null),
                json!({"decision_id":decision_id,"allowed":false,"reason":reason}),
            ),
        )
        .await?,
    )
    .await?;
    Err(ControlError::Denied {
        reason: reason.into(),
        decision_id,
    })
}
