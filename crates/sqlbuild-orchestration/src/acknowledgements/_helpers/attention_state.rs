//! What is already stored for a condition when a person acknowledges it or withdraws that.

use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::acknowledgements::constants::{ACKNOWLEDGED, ATTENTION_SNOOZE_HOURS, WITHDRAWN};
use crate::acknowledgements::models::AttentionAcknowledgementRequest;
use crate::acknowledgements::models::PriorAttention;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// The request's earlier application under its event ID, or else the condition's latest event.
pub(crate) async fn prior_attention(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    (request, body): (&AttentionAcknowledgementRequest, &Value),
) -> Result<(ControlTransaction, PriorAttention), ControlError> {
    let (transaction, retried) = query(
        transaction,
        "SELECT scope_json FROM sqb_security_audit WHERE event_id=$1",
        vec![Parameter::Id(request.event_id)],
    )
    .await?;
    if let Some(previous) = retried.first() {
        if previous["scope_json"].get("request") != Some(body) {
            return Err(ControlError::Invalid(
                "acknowledgement event ID already has different content".into(),
            ));
        }
        return Ok((transaction, PriorAttention::Retried));
    }
    let (transaction, latest) = query(
        transaction,
        &format!(
            "SELECT action, occurred_at, scope_json FROM sqb_security_audit \
             WHERE action IN ('{ACKNOWLEDGED}','{WITHDRAWN}') AND scope_json ->> 'location_id'=$1 \
             AND scope_json ->> 'target'=$2 AND scope_json ->> 'key'=$3 ORDER BY storage_id DESC LIMIT 1"
        ),
        vec![
            Parameter::Text(location_id.to_string()),
            Parameter::Text(target.into()),
            Parameter::Text(request.key.clone()),
        ],
    )
    .await?;
    let previous = latest.first();
    let at: Option<DateTime<Utc>> = previous
        .map(|row| serde_json::from_value(row["occurred_at"].clone()))
        .transpose()
        .map_err(|error| ControlError::Schema(format!("acknowledgement time: {error}")))?;
    let standing = PriorAttention::Standing {
        acknowledged: previous.is_some_and(|row| row["action"].as_str() == Some(ACKNOWLEDGED)),
        shape: previous.map_or(Value::Null, |row| row["scope_json"]["shape"].clone()),
        at,
    };
    Ok((transaction, standing))
}

/// Whether a request changes nothing: the same shape while it holds, or a withdrawal with none standing.
pub(crate) fn unchanged(
    (acknowledged, shape, at): (bool, &Value, Option<DateTime<Utc>>),
    request: &AttentionAcknowledgementRequest,
    now: DateTime<Utc>,
) -> bool {
    if !request.acknowledged {
        return !acknowledged;
    }
    acknowledged
        && *shape == request.shape
        && at.is_some_and(|at| now < at + Duration::hours(ATTENTION_SNOOZE_HOURS))
}
