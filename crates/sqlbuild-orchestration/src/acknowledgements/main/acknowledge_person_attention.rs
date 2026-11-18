//! Anyone who can see a location's runs marks an attention condition handled, as it looks now, as an
//! audit event; the same permission and retry rules as acknowledging a run.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::acknowledgements::constants::{
    ACKNOWLEDGED, MAX_KEY_CHARS, MAX_SHAPE_BYTES, WITHDRAWN, attention_snooze,
};
use crate::acknowledgements::models::AttentionAcknowledgementRequest;
use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{Change, ControlStore, Parameter, TransactionMode};

fn validate(request: &AttentionAcknowledgementRequest) -> Result<(), ControlError> {
    let key = request.key.as_str();
    if key.trim().is_empty()
        || key.chars().count() > MAX_KEY_CHARS
        || key.chars().any(char::is_control)
    {
        return Err(ControlError::Invalid(format!(
            "attention key must be 1 to {MAX_KEY_CHARS} characters without control characters"
        )));
    }
    if !request.shape.is_object() {
        return Err(ControlError::Invalid(
            "attention shape must be a JSON object".into(),
        ));
    }
    if request.shape.to_string().len() > MAX_SHAPE_BYTES {
        return Err(ControlError::Invalid(format!(
            "attention shape must be at most {MAX_SHAPE_BYTES} bytes"
        )));
    }
    Ok(())
}

/// Returns whether the condition's acknowledgement changed. Acknowledging again applies when the shape
/// differs or the last acknowledgement has lapsed; each change signals the target's runs, so open
/// Overviews reload.
pub async fn acknowledge_person_attention(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target, request): (Uuid, &str, &AttentionAcknowledgementRequest),
    default_role: DefaultRole,
) -> Result<bool, ControlError> {
    let (transaction, context) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    validate(request)?;
    let body =
        serde_json::to_value(request).map_err(|error| ControlError::Invalid(error.to_string()))?;
    let (transaction, retried) = query(
        transaction,
        "SELECT scope_json FROM sqb_security_audit WHERE event_id=$1",
        vec![Parameter::Id(request.event_id)],
    )
    .await?;
    if let Some(previous) = retried.first() {
        if previous["scope_json"].get("request") != Some(&body) {
            return Err(ControlError::Invalid(
                "acknowledgement event ID already has different content".into(),
            ));
        }
        commit(transaction).await?;
        return Ok(false);
    }
    let (transaction, latest) = query(
        transaction,
        &format!(
            "SELECT action, occurred_at, scope_json ->> 'shape' AS shape FROM sqb_security_audit \
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
    let acknowledged = previous.is_some_and(|row| row["action"].as_str() == Some(ACKNOWLEDGED));
    let shape: Value = previous
        .and_then(|row| row["shape"].as_str())
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or(Value::Null);
    let at = previous.and_then(|row| {
        serde_json::from_value::<chrono::DateTime<chrono::Utc>>(row["occurred_at"].clone()).ok()
    });
    let holding = acknowledged
        && shape == request.shape
        && at.is_some_and(|at| transaction.now < at + attention_snooze());
    // Acknowledging is a no-op while the same shape's acknowledgement holds; withdrawing, while none stands.
    let unchanged = if request.acknowledged {
        holding
    } else {
        !acknowledged
    };
    if unchanged {
        commit(transaction).await?;
        return Ok(false);
    }
    let action = if request.acknowledged {
        ACKNOWLEDGED
    } else {
        WITHDRAWN
    };
    let mut transaction = append_audit(
        transaction,
        &context,
        (request.event_id, action),
        (
            json!({"location_id": location_id, "target": target, "key": request.key, "shape": request.shape, "request": body}),
            json!({"acknowledged": acknowledged, "shape": shape}),
            json!({"acknowledged": request.acknowledged, "shape": request.shape}),
        ),
    )
    .await?;
    transaction
        .changes
        .insert(Change::Attention(location_id, target.into()));
    commit(transaction).await?;
    Ok(true)
}
