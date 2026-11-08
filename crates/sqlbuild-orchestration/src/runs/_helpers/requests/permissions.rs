//! Person authority covers the effective frozen execution, including sensitive group children.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::authorization::main::authorize_person::authorize_person;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::runs::_helpers::requests::job::frozen_group_children;
use crate::runs::models::{RunKind, RunRequest};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// Settings that override incremental windows; any of them rewrites history.
const CURSOR_SETTINGS: [&str; 6] = [
    "start_cursor",
    "end_cursor",
    "start_cursor_ts",
    "end_cursor_ts",
    "start_cursor_int",
    "end_cursor_int",
];

/// Confirmations of destructive changes an ad-hoc command may set; a published job's authored settings keep job.run.
const ADHOC_DESTRUCTIVE_SETTINGS: [&str; 4] = [
    "allow_snapshot_full_refresh",
    "allow_snapshot_schema_change",
    "allow_table_type_downgrade",
    "allow_retention_decrease",
];

pub(crate) async fn authorize_execution(
    mut transaction: ControlTransaction,
    (context, credentials): (&AuditContext, Uuid),
    (request, action): (&RunRequest, Action),
) -> Result<(ControlTransaction, AuditContext), ControlError> {
    let mut executions = vec![request.clone()];
    if request.kind == RunKind::Group {
        (transaction, executions) = frozen_group_children(transaction, request).await?;
    }
    let mut context = context.clone();
    for scope in execution_scopes(request, &executions, action) {
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

/// Every scope `action` on `request` needs, in check order; pure, so a dry run decides the same scopes without auditing.
pub(crate) fn execution_scopes(
    request: &RunRequest,
    executions: &[RunRequest],
    action: Action,
) -> Vec<PermissionScope> {
    let nodes: BTreeSet<String> = if request.kind == RunKind::Adhoc {
        request.selection.clone()
    } else {
        executions
            .iter()
            .flat_map(|run| run.write_set.iter().cloned())
            .collect()
    };
    let launches = matches!(action, Action::JobRun | Action::RunRerun | Action::RunAdhoc);
    let adhoc = request.kind == RunKind::Adhoc;
    let mut actions: Vec<Action> = vec![action];
    if action == Action::RunRerun && adhoc {
        actions.push(Action::RunAdhoc);
    }
    if launches && rewrites_history(request, executions) {
        actions.push(Action::RunRewriteHistory);
    }
    if launches && adhoc && request.settings.contains_key("warehouse") {
        actions.push(Action::RunWarehouse);
    }
    if launches && request.priority > 0 {
        actions.push(Action::RunPrioritise);
    }
    actions
        .into_iter()
        .map(|action| PermissionScope {
            action,
            location_id: Some(request.location_id),
            target: Some(request.target.clone()),
            nodes: nodes.clone(),
        })
        .collect()
}

fn rewrites_history(request: &RunRequest, executions: &[RunRequest]) -> bool {
    if request.window.is_some() {
        return true;
    }
    for run in executions {
        let mut names: Vec<&str> = vec!["full_refresh"];
        if run.kind == RunKind::Adhoc {
            names.extend(ADHOC_DESTRUCTIVE_SETTINGS);
        }
        let enabled = names
            .iter()
            .any(|name| run.settings.get(*name).and_then(serde_json::Value::as_bool) == Some(true));
        let cursor = CURSOR_SETTINGS
            .iter()
            .any(|name| run.settings.contains_key(*name));
        if enabled || cursor {
            return true;
        }
    }
    false
}
