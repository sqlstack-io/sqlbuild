//! Person authority covers the effective frozen execution, including sensitive group children.

use uuid::Uuid;

use crate::authorization::main::authorize_person::authorize_person;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::runs::_helpers::requests::job::frozen_group_children;
use crate::runs::models::{RunKind, RunRequest};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(crate) async fn authorize_execution(
    mut transaction: ControlTransaction,
    (context, credentials): (&AuditContext, Uuid),
    (request, action): (&RunRequest, Action),
) -> Result<(ControlTransaction, AuditContext), ControlError> {
    let mut executions = vec![request.clone()];
    if request.kind == RunKind::Group {
        (transaction, executions) = frozen_group_children(transaction, request).await?;
    }
    let mut scope = PermissionScope {
        action,
        location_id: Some(request.location_id),
        target: Some(request.target.clone()),
        nodes: executions
            .iter()
            .flat_map(|run| run.write_set.iter().cloned())
            .collect(),
    };
    let (mut transaction, mut context) = authorize_person(
        transaction,
        (context, credentials),
        &scope,
        DefaultRole::None,
    )
    .await?;
    if action == Action::RunRerun && request.kind == RunKind::Adhoc {
        scope.action = Action::RunAdhoc;
        (transaction, context) = authorize_person(
            transaction,
            (&context, credentials),
            &scope,
            DefaultRole::None,
        )
        .await?;
    }
    if matches!(action, Action::JobRun | Action::RunRerun)
        && (request.window.is_some()
            || executions.iter().any(|run| {
                run.settings
                    .get("full_refresh")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                    || run.settings.contains_key("start_cursor")
                    || run.settings.contains_key("end_cursor")
                    || run.settings.contains_key("start_cursor_ts")
                    || run.settings.contains_key("end_cursor_ts")
                    || run.settings.contains_key("start_cursor_int")
                    || run.settings.contains_key("end_cursor_int")
            }))
    {
        scope.action = Action::RunRewriteHistory;
        (transaction, context) = authorize_person(
            transaction,
            (&context, credentials),
            &scope,
            DefaultRole::None,
        )
        .await?;
    }
    Ok((transaction, context))
}
