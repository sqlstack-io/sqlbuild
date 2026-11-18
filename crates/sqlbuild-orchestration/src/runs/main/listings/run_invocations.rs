//! A run's Recall invocations: its scheduler's and, once claimed, its worker's; claim evidence outlives terminal publication.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// `scheduler/<run>`, then `worker/<claim>` when the scheduler recorded a claim.
pub(crate) async fn run_invocations(
    transaction: ControlTransaction,
    run_id: Uuid,
) -> Result<(ControlTransaction, Vec<String>), ControlError> {
    let scheduler = format!("scheduler/{run_id}");
    let (transaction, claims) = query(
        transaction,
        "SELECT payload FROM recall_events WHERE invocation_id=$1 AND event_type='run_claimed' ORDER BY invocation_sequence DESC LIMIT 1",
        vec![Parameter::Text(scheduler.clone())],
    )
    .await?;
    let mut namespaces = vec![scheduler];
    if let Some(claim_id) = claims
        .first()
        .and_then(|row| row["payload"]["claim"]["claim_id"].as_str())
    {
        namespaces.push(format!("worker/{claim_id}"));
    }
    Ok((transaction, namespaces))
}
