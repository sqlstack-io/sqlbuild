//! Manual job freezing, idempotence, authorization and publication share one transaction.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::main::authorize_session::authorize_session;
use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::runs::_helpers::requests::job::frozen_group_children;
use crate::runs::_helpers::requests::publication::{publish_request, request_retry};
use crate::runs::main::frozen_job_request::frozen_job_request;
use crate::runs::models::{JobRunRequest, RunKind, RunWindow};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(crate) async fn request_job_run_in(
    mut transaction: ControlTransaction,
    (context, credentials): (&AuditContext, Option<(&str, &str)>),
    request: &JobRunRequest,
) -> Result<(ControlTransaction, bool), ControlError> {
    let mut context = context.clone();
    let mut scope = PermissionScope {
        action: Action::JobRun,
        location_id: Some(request.location_id),
        target: Some(request.target.clone()),
        nodes: Default::default(),
    };
    if let Some(credentials) = credentials {
        (transaction, context) = authorize_session(
            transaction,
            (&context, credentials),
            &scope,
            DefaultRole::None,
        )
        .await?;
    }
    if context.actor.trim().is_empty()
        || context.granting_role.trim().is_empty()
        || request.target.trim().is_empty()
        || request.job_key.trim().is_empty()
        || request.reason.trim().is_empty()
    {
        return Err(ControlError::Invalid(
            "job run needs an actor, granting role, target, job and reason".into(),
        ));
    }
    if request.window.as_ref().is_some_and(|window| match window {
        RunWindow::Timestamp { start, end } => start > end,
        RunWindow::Integer { start, end } => start > end,
    }) {
        return Err(ControlError::Invalid(
            "backfill window start must not follow end".into(),
        ));
    }
    let event_id = Uuid::new_v5(&request.run_id, b"manual-job-request");
    let (transaction, retry) = request_retry(
        transaction,
        &context,
        (request.run_id, event_id, &json!(request)),
    )
    .await?;
    if retry {
        return Ok((transaction, false));
    }
    let (mut transaction, frozen) = frozen_job_request(transaction, &context, request).await?;
    if let Some(credentials) = credentials {
        let mut executions = vec![frozen.clone()];
        if frozen.kind == RunKind::Group {
            (transaction, executions) = frozen_group_children(transaction, &frozen).await?;
        }
        scope.nodes = executions
            .iter()
            .flat_map(|run| run.write_set.iter().cloned())
            .collect();
        if request.window.is_some()
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
            })
        {
            scope.action = Action::RunRewriteHistory;
            (transaction, context) = authorize_session(
                transaction,
                (&context, credentials),
                &scope,
                DefaultRole::None,
            )
            .await?;
        }
    }
    let (transaction, _) = publish_request(transaction, &frozen).await?;
    let transaction = append_audit(transaction, &context, (event_id, "job.run_requested"),
        (json!({"location_id":request.location_id,"target":request.target,"job_key":request.job_key,"run_id":request.run_id}),
         json!(null), json!({"request":request,"frozen":frozen}))).await?;
    Ok((transaction, true))
}
