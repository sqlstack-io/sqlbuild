//! A condition's acknowledgement is its latest acknowledgement event, projected from the audit log.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::acknowledgements::constants::{
    ACKNOWLEDGED, WITHDRAWN, attention_listing_window, attention_snooze,
};
use crate::acknowledgements::models::AttentionAcknowledgement;
use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Conditions whose latest event acknowledges them, from the last two days; a lapsed one is listed with
/// its `until`, so the Overview can say why the condition is back.
pub async fn list_person_attention_acknowledgements(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Vec<AttentionAcknowledgement>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let since = transaction.now - attention_listing_window();
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT a.action, COALESCE(p.name, a.actor) AS actor, a.occurred_at, a.scope_json ->> 'key' AS key, \
             a.scope_json ->> 'shape' AS shape \
             FROM sqb_security_audit a LEFT JOIN sqb_principals p ON 'principal:' || CAST(p.principal_id AS TEXT)=a.actor \
             WHERE a.occurred_at >= $1 AND a.action IN ('{ACKNOWLEDGED}','{WITHDRAWN}') \
             AND a.scope_json ->> 'location_id'=$2 AND a.scope_json ->> 'target'=$3 ORDER BY a.storage_id"
        ),
        vec![
            Parameter::Time(since),
            Parameter::Text(location_id.to_string()),
            Parameter::Text(target.into()),
        ],
    )
    .await?;
    commit(transaction).await?;
    let mut latest: BTreeMap<String, Option<AttentionAcknowledgement>> = BTreeMap::new();
    for row in rows {
        let key = row["key"].as_str().unwrap_or_default().to_owned();
        let acknowledged_at: DateTime<Utc> = serde_json::from_value(row["occurred_at"].clone())
            .map_err(|error| ControlError::Schema(format!("acknowledgement time: {error}")))?;
        let marked =
            (row["action"].as_str() == Some(ACKNOWLEDGED)).then(|| AttentionAcknowledgement {
                key: key.clone(),
                shape: row["shape"]
                    .as_str()
                    .and_then(|text| serde_json::from_str(text).ok())
                    .unwrap_or(Value::Null),
                acknowledged_by: row["actor"].as_str().unwrap_or_default().into(),
                acknowledged_at,
                until: acknowledged_at + attention_snooze(),
            });
        latest.insert(key, marked);
    }
    Ok(latest.into_values().flatten().collect())
}
