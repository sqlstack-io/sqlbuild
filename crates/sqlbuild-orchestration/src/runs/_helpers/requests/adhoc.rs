//! Resolve a command against the current publication and freeze it as an ad-hoc run request.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use uuid::Uuid;

use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::authorization::main::decisions::decide_person_scopes::decide_person_scopes;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{Action, AuditContext, DefaultRole};
use crate::runs::_helpers::lifecycle::publications::{cached_publication, current_publication};
use crate::runs::_helpers::requests::permissions::{authorize_execution, execution_scopes};
use crate::runs::_helpers::requests::publication::{
    publish_request, record_authored, request_retry,
};
use crate::runs::_helpers::requests::run_id::refuse_derived_run_id;
use crate::runs::constants::{ADHOC_AUDIT_TAG, LOAD_VERB};
use crate::runs::models::{
    AdhocCheckRequest, AdhocPermission, AdhocRunRequest, CachedPublication, CommandPlan,
    RunCommand, RunKind, RunOrigin, RunRequest,
};
use crate::runs::types::CommandPlanner;
use crate::snapshots::main::write_sets::held_write_set::held_write_set;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// The frozen request and its verb, or why the command cannot run; `run_id` comes from the caller.
pub(crate) async fn frozen_adhoc_request(
    transaction: ControlTransaction,
    (context, planner): (&AuditContext, &dyn CommandPlanner),
    (request, run_id): (&AdhocCheckRequest, Uuid),
) -> Result<(ControlTransaction, Result<(String, RunRequest), String>), ControlError> {
    let (transaction, current) =
        current_publication(transaction, planner, (request.location_id, &request.target)).await?;
    let Some((snapshot, publication)) = current else {
        return Ok((
            transaction,
            Err("this target has no publication yet".into()),
        ));
    };
    frozen_publication_request(
        transaction,
        context,
        (snapshot.snapshot_id, &publication),
        (request, run_id),
    )
    .await
}

/// [`frozen_adhoc_request`] over a publication the caller already holds.
pub(crate) async fn frozen_publication_request(
    transaction: ControlTransaction,
    context: &AuditContext,
    (snapshot_id, publication): (Uuid, &CachedPublication),
    (request, run_id): (&AdhocCheckRequest, Uuid),
) -> Result<(ControlTransaction, Result<(String, RunRequest), String>), ControlError> {
    let plan = match publication.prepared.plan(&request.command) {
        Ok(plan) => plan,
        Err(refusal) => return Ok((transaction, Err(refusal))),
    };
    let command = if plan.verb == LOAD_VERB {
        Some(RunCommand::Build)
    } else {
        RunCommand::of_verb(&plan.verb)
    };
    let Some(command) = command else {
        let refusal = format!("workers do not run sqb {}", plan.verb);
        return Ok((transaction, Err(refusal)));
    };
    let writers = held_write_set(&publication.writers, (&plan.selection, &plan.settings));
    let (transaction, images) = query(
        transaction,
        "SELECT image_id FROM sqb_snapshots WHERE snapshot_id=$1",
        vec![Parameter::Id(snapshot_id)],
    )
    .await?;
    let image_id = images
        .first()
        .and_then(|row| row["image_id"].as_str())
        .map(str::to_owned);
    let write_set: BTreeSet<String> = match plan.verb.as_str() {
        "build" | LOAD_VERB => writers,
        "seed" => writers
            .into_iter()
            .filter(|key| key.starts_with("seed:"))
            .collect(),
        _ => BTreeSet::new(),
    };
    Ok((
        transaction,
        Ok((
            plan.verb,
            RunRequest {
                run_id,
                location_id: request.location_id,
                target: request.target.clone(),
                snapshot_id,
                image_id,
                kind: RunKind::Adhoc,
                command,
                origin: RunOrigin::Manual,
                job_key: None,
                parent_run_id: None,
                root_run_id: run_id,
                trigger_id: None,
                tick_id: None,
                rerun_of_run_id: None,
                requested_by: context.actor.clone(),
                priority: request.priority,
                selection: plan.selection,
                write_set,
                settings: plan.settings,
                window: None,
                pools: BTreeMap::new(),
                narrowing: None,
                retry: None,
                member_attempt: None,
            },
        )),
    ))
}

/// `command` planned over one publication's nodes and its jobs, or why the planner refuses it.
pub(crate) async fn publication_plan(
    transaction: ControlTransaction,
    (planner, scope, snapshot_id): (&dyn CommandPlanner, (Uuid, &str), Uuid),
    command: &[String],
) -> Result<(ControlTransaction, Result<CommandPlan, String>), ControlError> {
    let (transaction, publication) =
        cached_publication(transaction, planner, (scope, snapshot_id)).await?;
    Ok((transaction, publication.prepared.plan(command)))
}

/// Every permission launching `frozen` needs, decided as the launch would but without recording a denial.
pub(in crate::runs) async fn permission_decisions(
    transaction: ControlTransaction,
    principal_id: Uuid,
    frozen: &RunRequest,
) -> Result<(ControlTransaction, Vec<AdhocPermission>), ControlError> {
    let scopes = execution_scopes(frozen, std::slice::from_ref(frozen), Action::RunAdhoc);
    let (transaction, refusals) =
        decide_person_scopes(transaction, principal_id, (&frozen.target, &scopes)).await?;
    let inactive = Some("authenticated active person required".to_owned());
    let refusals: Vec<Option<String>> = refusals.unwrap_or_else(|| vec![inactive; scopes.len()]);
    Ok((
        transaction,
        scopes
            .iter()
            .zip(refusals)
            .map(|(scope, reason)| AdhocPermission {
                action: scope.action,
                allowed: reason.is_none(),
                reason,
            })
            .collect(),
    ))
}

/// Resolve, authorise on the exact resolved nodes, publish and audit; workers never re-resolve the caller's selectors.
pub(in crate::runs) async fn request_adhoc_run_in(
    transaction: ControlTransaction,
    (context, principal_id): (&AuditContext, Uuid),
    (planner, request): (&dyn CommandPlanner, &AdhocRunRequest),
) -> Result<(ControlTransaction, bool), ControlError> {
    refuse_derived_run_id(request.run_id)?;
    let (transaction, context) = authorize_listing(
        transaction,
        (context, principal_id),
        (request.location_id, &request.target),
        DefaultRole::None,
    )
    .await?;
    if request.reason.trim().is_empty() || request.command.is_empty() {
        return Err(ControlError::Invalid(
            "an ad-hoc run needs a command and a reason".into(),
        ));
    }
    let authored = json!(request);
    let (transaction, retry) =
        request_retry(transaction, &context, (request.run_id, &authored)).await?;
    if retry {
        return Ok((transaction, false));
    }
    let check = AdhocCheckRequest {
        location_id: request.location_id,
        target: request.target.clone(),
        command: request.command.clone(),
        priority: request.priority,
    };
    let (transaction, frozen) =
        frozen_adhoc_request(transaction, (&context, planner), (&check, request.run_id)).await?;
    let (verb, frozen) = frozen.map_err(ControlError::Invalid)?;
    let (transaction, context) = authorize_execution(
        transaction,
        (&context, principal_id),
        (&frozen, Action::RunAdhoc),
    )
    .await?;
    let (transaction, _) = publish_request(transaction, &frozen).await?;
    let transaction = record_authored(transaction, &context, (request.run_id, &authored)).await?;
    let transaction = append_audit(
        transaction,
        &context,
        (Uuid::new_v5(&request.run_id, ADHOC_AUDIT_TAG), "run.adhoc_requested"),
        (
            json!({"location_id": request.location_id, "target": request.target, "run_id": request.run_id}),
            json!(null),
            json!({"request": request, "verb": verb, "frozen": frozen}),
        ),
    )
    .await?;
    Ok((transaction, true))
}
