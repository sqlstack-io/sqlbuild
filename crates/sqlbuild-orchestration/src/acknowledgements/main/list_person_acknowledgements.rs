//! A run's acknowledgement is its latest acknowledgement event, projected from the audit log.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::acknowledgements::models::RunAcknowledgement;
use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Runs marked handled in the last week; the overview lists only a day of problems.
pub async fn list_person_acknowledgements(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Vec<RunAcknowledgement>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let since = transaction.now - Duration::days(7);
    let (transaction, rows) = query(
        transaction,
        "SELECT a.action, COALESCE(p.name, a.actor) AS actor, a.occurred_at, a.scope_json ->> 'run_id' AS run_id \
         FROM sqb_security_audit a LEFT JOIN sqb_principals p ON 'principal:' || CAST(p.principal_id AS TEXT)=a.actor \
         WHERE a.occurred_at >= $1 AND a.action IN ('run.acknowledged','run.acknowledgement_withdrawn') \
         AND a.scope_json ->> 'location_id'=$2 AND a.scope_json ->> 'target'=$3 ORDER BY a.storage_id",
        vec![
            Parameter::Time(since),
            Parameter::Text(location_id.to_string()),
            Parameter::Text(target.into()),
        ],
    )
    .await?;
    commit(transaction).await?;
    let mut latest: BTreeMap<Uuid, Option<RunAcknowledgement>> = BTreeMap::new();
    for row in rows {
        let run_id: Uuid = serde_json::from_value(row["run_id"].clone())
            .map_err(|error| ControlError::Schema(format!("acknowledgement run: {error}")))?;
        let acknowledged_at: DateTime<Utc> = serde_json::from_value(row["occurred_at"].clone())
            .map_err(|error| ControlError::Schema(format!("acknowledgement time: {error}")))?;
        let marked =
            (row["action"].as_str() == Some("run.acknowledged")).then(|| RunAcknowledgement {
                run_id,
                acknowledged_by: row["actor"].as_str().unwrap_or_default().into(),
                acknowledged_at,
            });
        latest.insert(run_id, marked);
    }
    Ok(latest.into_values().flatten().collect())
}
