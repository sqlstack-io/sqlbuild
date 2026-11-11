//! Anyone who can see a location's runs marks one handled as an audit event; refusals precede the run lookup.

use serde_json::json;
use uuid::Uuid;

use crate::acknowledgements::models::AcknowledgementRequest;
use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{Change, ControlStore, Parameter, TransactionMode};

/// Returns whether the run's acknowledgement changed; each change signals the run, so open overviews reload.
pub async fn acknowledge_person_run(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target, request): (Uuid, &str, &AcknowledgementRequest),
    default_role: DefaultRole,
) -> Result<bool, ControlError> {
    let (transaction, context) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, runs) = query(
        transaction,
        "SELECT run_id FROM sqb_runs WHERE run_id=$1 AND location_id=$2 AND target=$3",
        vec![
            Parameter::Id(request.run_id),
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
        ],
    )
    .await?;
    if runs.is_empty() {
        return Err(ControlError::Invalid(
            "run does not exist in this location and target".into(),
        ));
    }
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
        "SELECT action FROM sqb_security_audit \
         WHERE action IN ('run.acknowledged','run.acknowledgement_withdrawn') AND scope_json ->> 'run_id'=$1 \
         ORDER BY storage_id DESC LIMIT 1",
        vec![Parameter::Text(request.run_id.to_string())],
    )
    .await?;
    let acknowledged = latest
        .first()
        .is_some_and(|row| row["action"].as_str() == Some("run.acknowledged"));
    if acknowledged == request.acknowledged {
        commit(transaction).await?;
        return Ok(false);
    }
    let action = if request.acknowledged {
        "run.acknowledged"
    } else {
        "run.acknowledgement_withdrawn"
    };
    let mut transaction = append_audit(
        transaction,
        &context,
        (request.event_id, action),
        (
            json!({"location_id": location_id, "target": target, "run_id": request.run_id, "request": body}),
            json!({"acknowledged": acknowledged}),
            json!({"acknowledged": request.acknowledged}),
        ),
    )
    .await?;
    transaction.changes.insert(Change::Run(request.run_id));
    commit(transaction).await?;
    Ok(true)
}
