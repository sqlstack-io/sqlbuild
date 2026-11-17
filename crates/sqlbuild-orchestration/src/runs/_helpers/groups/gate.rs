//! A member that waits per node runs each node once the nodes it reads from other members are built.
//!
//! An input counts as built once any attempt of its member built it, or when this group run does not
//! build it. It counts as failed only once its member's latest attempt ended, so a retry re-opens
//! the gate for the nodes it reruns.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use uuid::Uuid;

use crate::declarations::models::{DependencyFailure, GroupDefinition};
use crate::runs::_helpers::groups::context::load_group;
use crate::runs::models::{GateInput, GroupContext, NodeGate, RunRequest, RunStatus};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};

/// Where one input stands in this group run.
#[derive(Clone, Debug, PartialEq, Eq)]
enum InputState {
    Built,
    Pending,
    Failed(String),
}

/// One attempt of a member in this group run: its member, whether it ended, what it selects and whether a later attempt replaced it.
struct Attempt {
    job_key: String,
    terminal: bool,
    selection: BTreeSet<String>,
    superseded: bool,
}

/// A member attempt's row: whether it ended and whether a later attempt replaced it.
#[derive(Deserialize)]
struct AttemptRow {
    run_id: Uuid,
    request_json: RunRequest,
    status: RunStatus,
    superseded: i64,
}

/// One node's latest status in one attempt.
#[derive(Deserialize)]
struct NodeRow {
    run_id: Uuid,
    node_key: String,
    status: String,
    skipped_because_of: Option<String>,
}

/// The member's declaration and name, when `request` is a member of a group that waits per node.
fn per_node_member<'a>(
    group: &'a GroupContext,
    request: &'a RunRequest,
) -> Option<(&'a GroupDefinition, &'a str)> {
    let declaration = group
        .parent
        .request_json
        .job_key
        .as_ref()
        .and_then(|parent| group.definitions.groups.get(parent))?;
    let member = request.job_key.as_deref()?;
    declaration
        .waits_per_node(member)
        .then_some((declaration, member))
}

/// Each selected node's inputs from other members; under `on_failure = "skip"` every selected node, since any failed input skips the rest.
fn member_inputs(
    declaration: &GroupDefinition,
    member: &str,
    request: &RunRequest,
) -> BTreeMap<String, Vec<GateInput>> {
    let mut inputs: BTreeMap<String, Vec<GateInput>> = BTreeMap::new();
    if declaration.member_dependency_failure(member) == DependencyFailure::Skip {
        inputs.extend(
            request
                .selection
                .iter()
                .map(|node| (node.clone(), Vec::new())),
        );
    }
    for reason in declaration.wait_reasons.get(member).into_iter().flatten() {
        if !request.selection.contains(&reason.to_node) {
            continue;
        }
        let input = GateInput {
            job_key: reason.upstream_job.clone(),
            node_key: reason.from_node.clone(),
        };
        let node_inputs = inputs.entry(reason.to_node.clone()).or_default();
        if !node_inputs.contains(&input) {
            node_inputs.push(input);
        }
    }
    inputs
}

/// The gated nodes of a claimed group member and the inputs each waits for; empty for any other run.
pub(crate) async fn gated_nodes(
    transaction: ControlTransaction,
    request: &RunRequest,
) -> Result<(ControlTransaction, BTreeMap<String, Vec<GateInput>>), ControlError> {
    let Some(parent_id) = request.parent_run_id else {
        return Ok((transaction, BTreeMap::new()));
    };
    let (transaction, group) = load_group(transaction, parent_id).await?;
    let inputs = per_node_member(&group, request)
        .map(|(declaration, member)| member_inputs(declaration, member, request))
        .unwrap_or_default();
    Ok((transaction, inputs))
}

/// Each asked-about node's gate under its member's `on_failure` policy; a node without inputs is open.
pub(crate) async fn node_gates(
    transaction: ControlTransaction,
    request: &RunRequest,
    asked: &BTreeSet<String>,
) -> Result<(ControlTransaction, BTreeMap<String, NodeGate>), ControlError> {
    let open = || {
        asked
            .iter()
            .map(|node| (node.clone(), NodeGate::Open))
            .collect()
    };
    let Some(parent_id) = request.parent_run_id else {
        return Ok((transaction, open()));
    };
    let (transaction, group) = load_group(transaction, parent_id).await?;
    let Some((declaration, member)) = per_node_member(&group, request) else {
        return Ok((transaction, open()));
    };
    let inputs = member_inputs(declaration, member, request);
    let (transaction, states) = input_states(transaction, parent_id, &inputs).await?;
    let policy = declaration.member_dependency_failure(member);
    let failed_input = |node_inputs: &[GateInput]| {
        node_inputs
            .iter()
            .find_map(|input| match states.get(input) {
                Some(InputState::Failed(cause)) => Some(cause.clone()),
                _ => None,
            })
    };
    let member_failure = (policy == DependencyFailure::Skip)
        .then(|| {
            inputs
                .values()
                .find_map(|node_inputs| failed_input(node_inputs))
        })
        .flatten();
    let gates = asked
        .iter()
        .map(|node| {
            let node_inputs: &[GateInput] = inputs.get(node).map_or(&[], Vec::as_slice);
            let waiting_for: Vec<GateInput> = node_inputs
                .iter()
                .filter(|input| states.get(*input) == Some(&InputState::Pending))
                .cloned()
                .collect();
            let skipped_by = match policy {
                DependencyFailure::Run => None,
                DependencyFailure::Partial => failed_input(node_inputs),
                DependencyFailure::Skip => member_failure.clone(),
            };
            let gate = match skipped_by {
                Some(cause) => NodeGate::Skipped { cause },
                None if waiting_for.is_empty() => NodeGate::Open,
                None => NodeGate::Waiting { waiting_for },
            };
            (node.clone(), gate)
        })
        .collect();
    Ok((transaction, gates))
}

/// Where each input stands, from every attempt of every member of the group run.
async fn input_states(
    transaction: ControlTransaction,
    parent_id: Uuid,
    inputs: &BTreeMap<String, Vec<GateInput>>,
) -> Result<(ControlTransaction, BTreeMap<GateInput, InputState>), ControlError> {
    let (transaction, runs) = query(
        transaction,
        "SELECT r.run_id, r.request_json, r.status, \
         CASE WHEN EXISTS(SELECT 1 FROM sqb_runs s WHERE s.parent_run_id=r.parent_run_id AND s.rerun_of_run_id=r.run_id) THEN 1 ELSE 0 END AS superseded \
         FROM sqb_runs r WHERE r.parent_run_id=$1",
        vec![Parameter::Id(parent_id)],
    )
    .await?;
    let mut attempts: BTreeMap<Uuid, Attempt> = BTreeMap::new();
    for row in runs {
        let row: AttemptRow = decode_row(row)?;
        let Some(job_key) = row.request_json.job_key else {
            continue;
        };
        attempts.insert(
            row.run_id,
            Attempt {
                job_key,
                terminal: row.status.is_terminal(),
                selection: row.request_json.selection,
                superseded: row.superseded != 0,
            },
        );
    }
    let (transaction, rows) = query(
        transaction,
        "SELECT n.run_id, n.node_key, n.status, n.skipped_because_of FROM sqb_run_nodes n \
         JOIN sqb_runs r ON r.run_id=n.run_id WHERE r.parent_run_id=$1 AND n.kind='node'",
        vec![Parameter::Id(parent_id)],
    )
    .await?;
    let mut built: BTreeSet<(String, String)> = BTreeSet::new();
    let mut failed: BTreeMap<(String, String), String> = BTreeMap::new();
    for row in rows {
        let row: NodeRow = decode_row(row)?;
        let Some(attempt) = attempts.get(&row.run_id) else {
            continue;
        };
        let key = (attempt.job_key.clone(), row.node_key.clone());
        match row.status.as_str() {
            "success" | "warn" => {
                built.insert(key);
            }
            "failed" | "skipped" | "cancelled" if !attempt.superseded => {
                failed.insert(key, row.skipped_because_of.unwrap_or(row.node_key));
            }
            _ => {}
        }
    }
    let states = inputs
        .values()
        .flatten()
        .map(|input| {
            let key = (input.job_key.clone(), input.node_key.clone());
            let member_attempts: Vec<&Attempt> = attempts
                .values()
                .filter(|attempt| {
                    attempt.job_key == input.job_key && attempt.selection.contains(&input.node_key)
                })
                .collect();
            let latest_ended = member_attempts
                .iter()
                .filter(|attempt| !attempt.superseded)
                .all(|attempt| attempt.terminal);
            let state = if built.contains(&key) || member_attempts.is_empty() {
                InputState::Built
            } else if !latest_ended {
                InputState::Pending
            } else {
                InputState::Failed(
                    failed
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| input.node_key.clone()),
                )
            };
            (input.clone(), state)
        })
        .collect();
    Ok((transaction, states))
}
