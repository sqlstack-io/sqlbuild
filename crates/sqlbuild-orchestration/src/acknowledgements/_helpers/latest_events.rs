//! An acknowledgement is its subject's latest acknowledging or withdrawing audit event.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::acknowledgements::models::{
    AcknowledgementEvents, AcknowledgementListing, LatestAcknowledgement,
};
use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// A target's acknowledgement events since `$1` with the actor's name, by subquery so the index serves it.
pub(crate) fn listing_sql(events: &AcknowledgementEvents) -> String {
    format!(
        "SELECT a.event_id, a.action, \
         COALESCE((SELECT p.name FROM sqb_principals p WHERE 'principal:' || CAST(p.principal_id AS TEXT)=a.actor), a.actor) AS actor, \
         a.occurred_at, a.scope_json FROM sqb_security_audit a \
         WHERE a.action IN ('{}', '{}') AND a.occurred_at >= $1 \
         AND a.scope_json ->> 'location_id'=$2 AND a.scope_json ->> 'target'=$3 ORDER BY a.storage_id",
        events.acknowledged, events.withdrawn
    )
}

/// Subjects whose latest event in the window acknowledges them, for a person who can list the runs.
pub(crate) async fn latest_acknowledgements(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    listing: AcknowledgementListing<'_>,
) -> Result<Vec<LatestAcknowledgement>, ControlError> {
    let AcknowledgementListing {
        location_id,
        target,
        default_role,
        events,
        window,
    } = listing;
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let since = window.map_or(DateTime::UNIX_EPOCH, |window| transaction.now - window);
    let (transaction, rows) = query(
        transaction,
        &listing_sql(events),
        vec![
            Parameter::Time(since),
            Parameter::Text(location_id.to_string()),
            Parameter::Text(target.into()),
        ],
    )
    .await?;
    commit(transaction).await?;
    let mut latest: BTreeMap<String, Option<LatestAcknowledgement>> = BTreeMap::new();
    for row in rows {
        let scope = row["scope_json"].clone();
        let subject = scope[events.subject]
            .as_str()
            .ok_or_else(|| {
                ControlError::Schema(format!("acknowledgement without its {}", events.subject))
            })?
            .to_owned();
        let acknowledged_at: DateTime<Utc> = serde_json::from_value(row["occurred_at"].clone())
            .map_err(|error| ControlError::Schema(format!("acknowledgement time: {error}")))?;
        let event_id: Uuid = serde_json::from_value(row["event_id"].clone())
            .map_err(|error| ControlError::Schema(format!("acknowledgement event: {error}")))?;
        let marked =
            (row["action"].as_str() == Some(events.acknowledged)).then(|| LatestAcknowledgement {
                event_id,
                subject: subject.clone(),
                acknowledged_by: row["actor"].as_str().unwrap_or_default().into(),
                acknowledged_at,
                scope,
            });
        latest.insert(subject, marked);
    }
    Ok(latest.into_values().flatten().collect())
}
