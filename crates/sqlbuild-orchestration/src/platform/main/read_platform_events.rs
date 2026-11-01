//! One authoritative namespace filter protects every platform stream consumer from worker input.

use crate::platform::constants::PLATFORM_EVENT_PAGE_SIZE;
use crate::platform::models::PlatformEventEnvelope;
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn read_platform_events(
    transaction: ControlTransaction,
    after: i64,
) -> Result<(ControlTransaction, Vec<PlatformEventEnvelope>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT storage_id,event_id,occurred_at,payload FROM recall_events WHERE producer='platform' \
         AND invocation_id LIKE 'platform/%' AND storage_id>$1 ORDER BY storage_id LIMIT $2",
        vec![Parameter::Integer(after), Parameter::Integer(PLATFORM_EVENT_PAGE_SIZE)]).await?;
    Ok((
        transaction,
        rows.into_iter().map(decode_row).collect::<Result<_, _>>()?,
    ))
}
