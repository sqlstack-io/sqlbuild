//! An acknowledgement is the latest acknowledging or withdrawing event of its subject, projected from the
//! audit log; runs and attention conditions are both listed this way.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// The audit actions of one kind of acknowledgement, and the scope field naming its subject.
pub(crate) struct AcknowledgementEvents {
    pub(crate) acknowledged: &'static str,
    pub(crate) withdrawn: &'static str,
    pub(crate) subject: &'static str,
}

/// A subject's standing acknowledgement: who made it, when, and the event's scope.
pub(crate) struct LatestAcknowledgement {
    pub(crate) subject: String,
    pub(crate) acknowledged_by: String,
    pub(crate) acknowledged_at: DateTime<Utc>,
    pub(crate) scope: Value,
}

/// Subjects of the target whose latest event within `window` acknowledges them, by subject. The person
/// must be able to list the target's runs.
pub(crate) async fn latest_acknowledgements(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
    (events, window): (&AcknowledgementEvents, Duration),
) -> Result<Vec<LatestAcknowledgement>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let since = transaction.now - window;
    let (transaction, rows) = query(
        transaction,
        "SELECT a.action, COALESCE(p.name, a.actor) AS actor, a.occurred_at, a.scope_json \
         FROM sqb_security_audit a LEFT JOIN sqb_principals p ON 'principal:' || CAST(p.principal_id AS TEXT)=a.actor \
         WHERE a.occurred_at >= $1 AND a.action IN ($2, $3) \
         AND a.scope_json ->> 'location_id'=$4 AND a.scope_json ->> 'target'=$5 ORDER BY a.storage_id",
        vec![
            Parameter::Time(since),
            Parameter::Text(events.acknowledged.into()),
            Parameter::Text(events.withdrawn.into()),
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
        let marked =
            (row["action"].as_str() == Some(events.acknowledged)).then(|| LatestAcknowledgement {
                subject: subject.clone(),
                acknowledged_by: row["actor"].as_str().unwrap_or_default().into(),
                acknowledged_at,
                scope,
            });
        latest.insert(subject, marked);
    }
    Ok(latest.into_values().flatten().collect())
}
