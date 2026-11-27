//! Source-specific readers retain authority; delivery owns only acknowledgements and skipping a platform event that can't be read.

use crate::credentials::main::audit::read_audit_events::read_audit_events;
use crate::platform::main::installation::read_platform_events::read_platform_events;
use crate::platform::models::UnreadableRow;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;
use crate::webhooks::models::{WebhookEvent, WebhookSource};

pub(crate) async fn read_events(
    transaction: ControlTransaction,
    source: WebhookSource,
    after: i64,
) -> Result<
    (
        ControlTransaction,
        Vec<Result<WebhookEvent, Box<UnreadableRow>>>,
    ),
    ControlError,
> {
    match source {
        WebhookSource::Platform => {
            let (transaction, events) = read_platform_events(transaction, after).await?;
            Ok((
                transaction,
                events
                    .into_iter()
                    .map(|event| event.map(WebhookEvent::Platform))
                    .collect(),
            ))
        }
        WebhookSource::SecurityAudit => {
            let (transaction, events) =
                read_audit_events(transaction, ((after, None), 100)).await?;
            Ok((
                transaction,
                events
                    .into_iter()
                    .map(Box::new)
                    .map(WebhookEvent::SecurityAudit)
                    .map(Ok)
                    .collect(),
            ))
        }
    }
}
