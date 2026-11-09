//! Resolve a command against the current publication and freeze it as an ad-hoc run request.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::authorization::main::decide_person_scopes::decide_person_scopes;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::declarations::main::frozen_definitions::frozen_definitions;
use crate::runs::_helpers::requests::permissions::{authorize_execution, execution_scopes};
use crate::runs::_helpers::requests::publication::{
    publish_request, record_authored, request_retry,
};
use crate::runs::constants::ADHOC_AUDIT_TAG;
use crate::runs::models::{
    AdhocCheckRequest, AdhocPermission, AdhocRunRequest, PlannerResource, RunCommand, RunKind,
    RunOrigin, RunRequest,
};
use crate::runs::types::{CommandPlanner, PublishedJobs};
use crate::snapshots::main::job_write_set::job_write_set;
use crate::snapshots::main::read_current_record::read_current_record;
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Graph kinds in the order the CLI builds its project graph: models, sources, seeds, then functions.
const GRAPH_KINDS: [&str; 5] = ["model", "source", "seed", "udf", "table_fn"];

/// The frozen request and its verb, or why the command cannot run; `run_id` comes from the caller.
pub(crate) async fn frozen_adhoc_request(
    transaction: ControlTransaction,
    (context, planner): (&AuditContext, &dyn CommandPlanner),
    (request, run_id): (&AdhocCheckRequest, Uuid),
) -> Result<(ControlTransaction, Result<(String, RunRequest), String>), ControlError> {
    let (transaction, snapshot) =
        read_current_record(transaction, (request.location_id, &request.target)).await?;
    let Some(snapshot) = snapshot else {
        return Ok((
            transaction,
            Err("this target has no publication yet".into()),
        ));
    };
    let (transaction, nodes) = read_snapshot_nodes(transaction, snapshot.snapshot_id, None).await?;
    let (transaction, definitions) = frozen_definitions(transaction, snapshot.snapshot_id).await?;
    let jobs: PublishedJobs = definitions
        .jobs
        .into_iter()
        .map(|(name, job)| (name, job.selection))
        .chain(
            definitions
                .groups
                .into_iter()
                .map(|(name, group)| (name, group.job.selection)),
        )
        .collect();
    let plan = match planner.plan(&request.command, (&planner_resources(&nodes), &jobs)) {
        Ok(plan) => plan,
        Err(refusal) => return Ok((transaction, Err(refusal))),
    };
    let Some(command) = RunCommand::of_verb(&plan.verb) else {
        let refusal = format!("workers do not run sqb {}", plan.verb);
        return Ok((transaction, Err(refusal)));
    };
    let (transaction, writers) =
        job_write_set(transaction, snapshot.snapshot_id, &plan.selection).await?;
    let (transaction, images) = query(
        transaction,
        "SELECT image_id FROM sqb_snapshots WHERE snapshot_id=$1",
        vec![Parameter::Id(snapshot.snapshot_id)],
    )
    .await?;
    let image_id = images
        .first()
        .and_then(|row| row["image_id"].as_str())
        .map(str::to_owned);
    let write_set: BTreeSet<String> = match plan.verb.as_str() {
        "build" => writers,
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
                snapshot_id: snapshot.snapshot_id,
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
                concurrency_groups: BTreeSet::new(),
            },
        )),
    ))
}

/// The selector graph rebuilt from published definitions, in the CLI's kind order.
fn planner_resources(
    nodes: &std::collections::BTreeMap<String, SnapshotNode>,
) -> Vec<PlannerResource> {
    let mut resources: Vec<PlannerResource> = Vec::new();
    for kind in GRAPH_KINDS {
        for (key, node) in nodes {
            if node.kind != kind {
                continue;
            }
            let definition = &node.definition;
            let (tags, folder) = match kind {
                "model" => (
                    definition.pointer("/config/tags"),
                    Some(model_folder(definition)),
                ),
                "seed" => (definition.pointer("/schema/tags"), None),
                _ => (definition.get("tags"), None),
            };
            resources.push(PlannerResource {
                key: key.clone(),
                deps: texts(definition.get("deps")),
                tags: texts(tags),
                folder,
            });
        }
    }
    resources
}

/// The model file's folder relative to the project, with `/` separators whichever platform published it.
fn model_folder(definition: &Value) -> String {
    let path = definition
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .replace('\\', "/");
    path.rsplit_once('/')
        .map_or_else(|| ".".to_owned(), |(folder, _)| folder.to_owned())
}

fn texts(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}

/// Whether the person may view every current node, decided without recording a denial.
pub(in crate::runs) async fn may_view_target(
    transaction: ControlTransaction,
    principal_id: Uuid,
    request: &AdhocCheckRequest,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (transaction, current) = query(transaction,
        "SELECT n.node_key FROM sqb_snapshot_nodes n JOIN sqb_current_snapshots c ON c.snapshot_id=n.snapshot_id \
         WHERE c.location_id=$1 AND c.target=$2",
        vec![Parameter::Id(request.location_id), Parameter::Text(request.target.clone())]).await?;
    let view = PermissionScope {
        action: Action::View,
        location_id: Some(request.location_id),
        target: Some(request.target.clone()),
        nodes: current
            .iter()
            .filter_map(|row| row["node_key"].as_str().map(str::to_owned))
            .collect(),
    };
    let (transaction, refusals) =
        decide_person_scopes(transaction, principal_id, (&request.target, &[view])).await?;
    Ok((transaction, refusals.as_deref() == Some(&[None])))
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
