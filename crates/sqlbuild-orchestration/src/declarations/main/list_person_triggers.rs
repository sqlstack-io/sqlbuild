//! A person lists triggers only with view on every current node of the target.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::declarations::_helpers::reads::{latest_ticks_in, triggers_in};
use crate::declarations::models::PersonTriggerRecord;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn list_person_triggers(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Vec<PersonTriggerRecord>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, triggers) = triggers_in(transaction, (location_id, target)).await?;
    let (transaction, ticks) = latest_ticks_in(transaction, (location_id, target)).await?;
    commit(transaction).await?;
    let mut latest: BTreeMap<Uuid, _> = ticks
        .into_iter()
        .map(|tick| (tick.trigger_id, tick))
        .collect();
    Ok(triggers
        .into_iter()
        .map(|trigger| PersonTriggerRecord {
            latest_tick: latest.remove(&trigger.trigger_id),
            trigger,
        })
        .collect())
}
