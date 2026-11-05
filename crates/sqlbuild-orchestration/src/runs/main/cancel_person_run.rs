//! Cancellation derives scope and actor from durable execution inputs and live authentication.

use uuid::Uuid;

use crate::authorization::main::authorize_person::authorize_person;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::runs::_helpers::lifecycle::change_run::change_run_in;
use crate::runs::_helpers::requests::permissions::authorize_execution;
use crate::runs::models::{
    CancelRunRequest, RunAuthority, RunChange, RunChangeRequest, RunRequest,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn cancel_person_run(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    request: &CancelRunRequest,
) -> Result<bool, ControlError> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Write).await?,
        "SELECT request_json FROM sqb_runs WHERE run_id=$1",
        vec![Parameter::Id(request.run_id)],
    )
    .await?;
    let Some(source) = rows.into_iter().next() else {
        authorize_person(
            transaction,
            credentials,
            &PermissionScope {
                action: Action::RunCancel,
                location_id: None,
                target: None,
                nodes: Default::default(),
            },
            DefaultRole::None,
        )
        .await?;
        return Err(ControlError::Invalid("run does not exist".into()));
    };
    let frozen: RunRequest = serde_json::from_value(source["request_json"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let (transaction, context) =
        authorize_execution(transaction, credentials, (&frozen, Action::RunCancel)).await?;
    let (transaction, applied) = change_run_in(
        transaction,
        RunAuthority::Operator(&context),
        &RunChangeRequest {
            run_id: request.run_id,
            event_id: request.event_id,
            claim_id: None,
            change: RunChange::CancelRequested {
                actor: context.actor.clone(),
                reason: request.reason.clone(),
            },
        },
    )
    .await?;
    commit(transaction).await?;
    Ok(applied)
}
