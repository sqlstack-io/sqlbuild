//! What is already stored for a condition when a person acknowledges it or withdraws that.

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::acknowledgements::constants::{ACKNOWLEDGED, WITHDRAWN};
use crate::acknowledgements::models::PriorAttention;
use crate::acknowledgements::models::{
    AttentionAcknowledgementRequest, AttentionSnooze, StandingAttention,
};
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
            "SELECT event_id, action, occurred_at, scope_json FROM sqb_security_audit \
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
    let event_id: Option<Uuid> = previous
        .map(|row| serde_json::from_value(row["event_id"].clone()))
        .transpose()
        .map_err(|error| ControlError::Schema(format!("acknowledgement event: {error}")))?;
    let standing = StandingAttention {
        event_id,
        acknowledged: previous.is_some_and(|row| row["action"].as_str() == Some(ACKNOWLEDGED)),
        shape: previous.map_or(Value::Null, |row| row["scope_json"]["shape"].clone()),
        at,
    };
    Ok((transaction, PriorAttention::Standing(standing)))
}

/// Whether a request changes nothing: a held shape, or a withdrawal of nothing or of a superseded acknowledgement.
pub(crate) fn unchanged(
    standing: &StandingAttention,
    request: &AttentionAcknowledgementRequest,
    (now, snooze): (DateTime<Utc>, AttentionSnooze),
) -> bool {
    if !request.acknowledged {
        return !standing.acknowledged
            || request
                .withdraws
                .is_some_and(|named| standing.event_id != Some(named));
    }
    standing.acknowledged
        && standing.shape == request.shape
        && standing
            .at
            .is_some_and(|at| snooze.until(at).is_none_or(|until| now < until))
}
