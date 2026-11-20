//! Person authority covers the effective frozen execution, including sensitive group children.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::authorization::main::decisions::authorize_person::authorize_person;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::runs::_helpers::lifecycle::pinned_jobs::{
    applied_settings, applying_jobs, gives_build_settings,
};
use crate::runs::_helpers::requests::job::frozen_group_children;
use crate::runs::constants::VARS_SETTING;
use crate::runs::models::{RunKind, RunRequest};
use crate::runs::types::ExecutionScope;
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

/// Settings that ask for source loads beyond a plain build's automatic ones; the `load` verb sets `load_sources`.
const EXPLICIT_LOAD_SETTINGS: [&str; 2] = ["load_sources", "reload"];

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
    for (scope, job) in execution_scopes(request, &executions, action) {
        (transaction, context) = authorize_person(
            transaction,
            (&context, credentials),
            &scope,
            DefaultRole::None,
        )
        .await
        .map_err(|error| match (error, &job) {
            (
                ControlError::Denied {
                    reason,
                    decision_id,
                },
                Some(job),
            ) => ControlError::Denied {
                reason: job_denial(job, &reason),
                decision_id,
            },
            (error, _) => error,
        })?;
    }
    Ok((transaction, context))
}

/// Why a person may not apply a pinned job's settings: `reason`, naming the job and the permission.
pub(in crate::runs) fn job_denial(job: &str, reason: &str) -> String {
    format!("{reason}: applying job {job}'s settings needs job.run, as running the job does")
}

/// Every scope `action` on `request` needs, in check order, each with the pinned job it applies for; pure, so a dry run decides the same scopes without auditing.
pub(crate) fn execution_scopes(
    request: &RunRequest,
    executions: &[RunRequest],
    action: Action,
) -> Vec<ExecutionScope> {
    let nodes: BTreeSet<String> = if request.kind == RunKind::Adhoc {
        request
            .selection
            .union(&request.write_set)
            .cloned()
            .collect()
    } else {
        executions
            .iter()
            .flat_map(|run| run.write_set.iter().cloned())
            .collect()
    };
    let launches = matches!(action, Action::JobRun | Action::RunRerun | Action::RunAdhoc);
    let adhoc = request.kind == RunKind::Adhoc;
    let scope = |action: Action, nodes: BTreeSet<String>| PermissionScope {
        action,
        location_id: Some(request.location_id),
        target: Some(request.target.clone()),
        nodes,
    };
    let mut scopes: Vec<ExecutionScope> = vec![(scope(action, nodes.clone()), None)];
    if action == Action::RunRerun && adhoc {
        scopes.push((scope(Action::RunAdhoc, nodes.clone()), None));
    }
    if launches && adhoc {
        for (name, job) in applying_jobs(request) {
            let mut held: BTreeSet<String> = job.selection.clone();
            if gives_build_settings(request, job) {
                held.extend(nodes.iter().cloned());
            }
            scopes.push((scope(Action::JobRun, held), Some(name.clone())));
        }
    }
    let mut actions: Vec<Action> = Vec::new();
    if launches && rewrites_history(request, executions) {
        actions.push(Action::RunRewriteHistory);
    }
    if launches && adhoc && request.settings.contains_key("warehouse") {
        actions.push(Action::RunWarehouse);
    }
    if launches && adhoc && request.settings.contains_key(VARS_SETTING) {
        actions.push(Action::RunVars);
    }
    if launches && request.priority > 0 {
        actions.push(Action::RunPrioritise);
    }
    scopes.extend(
        actions
            .into_iter()
            .map(|action| (scope(action, nodes.clone()), None)),
    );
    let loaded = explicitly_loaded(request);
    if launches && adhoc && !loaded.is_empty() {
        scopes.push((scope(Action::RunLoad, loaded), None));
    }
    scopes
}

/// The sources an ad-hoc `--load`, `--reload` or `load`, typed or from a pinned job, writes; a plain build's automatic loads need only `run.adhoc`.
fn explicitly_loaded(request: &RunRequest) -> BTreeSet<String> {
    let applied = applied_settings(request);
    let explicit = EXPLICIT_LOAD_SETTINGS
        .iter()
        .any(|name| applied.get(*name).and_then(serde_json::Value::as_bool) == Some(true));
    if !explicit {
        return BTreeSet::new();
    }
    request
        .write_set
        .iter()
        .filter(|key| key.starts_with("source:"))
        .cloned()
        .collect()
}

/// A window, a full refresh, a cursor setting or an ad-hoc destructive flag on any of `executions`, typed or from an ad-hoc build's pinned jobs.
pub(in crate::runs) fn rewrites_history(request: &RunRequest, executions: &[RunRequest]) -> bool {
    if request.window.is_some() {
        return true;
    }
    for run in executions {
        let mut names: Vec<&str> = vec!["full_refresh"];
        if run.kind == RunKind::Adhoc {
            names.extend(ADHOC_DESTRUCTIVE_SETTINGS);
        }
        let settings = applied_settings(run);
        let enabled = names
            .iter()
            .any(|name| settings.get(*name).and_then(serde_json::Value::as_bool) == Some(true));
        let cursor = CURSOR_SETTINGS
            .iter()
            .any(|name| settings.contains_key(*name));
        if enabled || cursor {
            return true;
        }
    }
    false
}
