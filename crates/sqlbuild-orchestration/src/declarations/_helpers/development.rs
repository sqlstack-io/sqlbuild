//! Development safety defaults are ordinary audited trigger changes, not edited definitions.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::declarations::_helpers::operator_change::change_trigger_in;
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn trigger_ids(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, BTreeSet<Uuid>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT trigger_id FROM sqb_triggers WHERE location_id=$1 AND target=$2 AND removed_in_snapshot_id IS NULL",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())],
    ).await?;
    let ids = rows
        .iter()
        .map(|row| {
            row["trigger_id"]
                .as_str()
                .unwrap_or_default()
                .parse()
                .map_err(|_| ControlError::Schema("invalid trigger identity".into()))
        })
        .collect::<Result<_, _>>()?;
    Ok((transaction, ids))
}

pub(crate) async fn pause_triggers(
    transaction: ControlTransaction,
    scope: (Uuid, &str),
    previous: &BTreeSet<Uuid>,
    request_id: Uuid,
) -> Result<ControlTransaction, ControlError> {
    let (mut transaction, ids) = trigger_ids(transaction, scope).await?;
    let context = AuditContext {
        actor: "local-development".into(),
        granting_role: "control-store-owner".into(),
        request_id,
        ip_address: None,
    };
    for trigger_id in ids.difference(previous) {
        (transaction, _) = change_trigger_in(
            transaction,
            &context,
            &TriggerChangeRequest {
                event_id: Uuid::new_v5(&request_id, trigger_id.as_bytes()),
                trigger_id: *trigger_id,
                actor: context.actor.clone(),
                reason: "Development starts with triggers paused".into(),
                change: TriggerChange::Pause,
            },
        )
        .await?;
    }
    Ok(transaction)
}
