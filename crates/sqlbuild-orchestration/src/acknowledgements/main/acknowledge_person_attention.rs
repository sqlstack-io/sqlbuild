//! Anyone who can see a target's runs marks a condition handled as it looks now, as an audit event.

use serde_json::json;
use uuid::Uuid;

use crate::acknowledgements::_helpers::attention_state::{prior_attention, unchanged};
use crate::acknowledgements::constants::{ACKNOWLEDGED, MAX_KEY_CHARS, MAX_SHAPE_BYTES, WITHDRAWN};
use crate::acknowledgements::models::AttentionAcknowledgementRequest;
use crate::acknowledgements::models::PriorAttention;
use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{Change, ControlStore, TransactionMode};

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

/// Whether the acknowledgement changed; the same shape again applies only once the last one lapsed.
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
    let (transaction, prior) =
        prior_attention(transaction, (location_id, target), (request, &body)).await?;
    let PriorAttention::Standing {
        acknowledged,
        shape,
        at,
    } = prior
    else {
        commit(transaction).await?;
        return Ok(false);
    };
    if unchanged((acknowledged, &shape, at), request, transaction.now) {
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
