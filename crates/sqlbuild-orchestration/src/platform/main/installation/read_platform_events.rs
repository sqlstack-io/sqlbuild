//! One authoritative namespace filter protects every platform stream consumer from worker input; each row is read on its own, so one that can't be read never stops the stream.

use crate::platform::_helpers::health::event_history::read_event;
use crate::platform::constants::PLATFORM_EVENT_PAGE_SIZE;
use crate::platform::models::{PlatformEventEnvelope, StoredPlatformEvent, UnreadableRow};
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};

/// The next page of platform events after `after` in storage order: each one read, or what is known about one that can't be.
pub(crate) async fn read_platform_events(
    transaction: ControlTransaction,
    after: i64,
) -> Result<
    (
        ControlTransaction,
        Vec<Result<PlatformEventEnvelope, Box<UnreadableRow>>>,
    ),
    ControlError,
> {
    let (transaction, rows): (ControlTransaction, Vec<StoredPlatformEvent>) = query_records(transaction,
        "SELECT storage_id,event_id,occurred_at,event_type,payload FROM recall_events WHERE producer='platform' \
         AND invocation_id LIKE 'platform/%' AND storage_id>$1 ORDER BY storage_id LIMIT $2",
        vec![Parameter::Integer(after), Parameter::Integer(PLATFORM_EVENT_PAGE_SIZE)]).await?;
    Ok((transaction, rows.into_iter().map(read_event).collect()))
}
