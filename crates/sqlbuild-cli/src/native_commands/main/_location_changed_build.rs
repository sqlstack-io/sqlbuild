//! After each publish and each completed sync the location plans the web UI's "Build what changed" with the plan page's own path, from the synced copy, and reports what it runs.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;

use serde_json::Value;
use sqlbuild_orchestration::node_states::models::ChangedBuildReport;
use sqlbuild_orchestration::platform::models::LocationWorkClient;
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::adhoc::_helpers::planner::command_settings;
use crate::argv::models::Stream;
use crate::native_commands::_helpers::paths::location_files;
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::constants::CHANGED_BUILD_COMMAND;
use crate::native_commands::main::_location_plan::plan_location;
use crate::native_commands::models::{LocationRuntimeRequest, LocationServe, LocationWatch};

/// The settings the server freezes a plan request of `sqb build --changed` to; with no selector the plan selects every node, as the frozen selection does.
pub(in crate::native_commands) fn changed_build_settings() -> Result<BTreeMap<String, Value>, String>
{
    command_settings(&CHANGED_BUILD_COMMAND.map(str::to_owned))
}

/// The nodes a `sqb plan --json --changed` document runs: for a change of their own, and only because an upstream runs; or why it has none.
pub(in crate::native_commands) fn changed_build_nodes(
    document: &Value,
) -> Result<(Vec<String>, Vec<String>), String> {
    if let Some(message) = document["error"]["message"].as_str() {
        return Err(message.to_owned());
    }
    let runs = document["changed"]["runs"]
        .as_array()
        .ok_or_else(|| "the plan has no --changed section".to_owned())?;
    let mut changed: Vec<String> = Vec::new();
    let mut downstream: Vec<String> = Vec::new();
    for run in runs {
        let node = run["node"]
            .as_str()
            .ok_or_else(|| "a planned run names no node".to_owned())?
            .to_owned();
        if run["downstream_of"].is_null() {
            changed.push(node);
        } else {
            downstream.push(node);
        }
    }
    Ok((changed, downstream))
}

/// Plan and report; whether the server took the count, or `None` when stopped first.
pub(super) async fn report_changed_build(
    (options, runtime, watch): (
        &LocationServe,
        &mpsc::Sender<LocationRuntimeRequest>,
        &LocationWatch,
    ),
    (client, session_id, generation): (&LocationWorkClient, Uuid, i64),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<bool>, ControlError> {
    let published = || {
        location_files(&options.project_dir, &watch.rules, &watch.local_files) == watch.observed.0
    };
    let report = |message: &str| drop(write_output(&[(Stream::Stderr, message)]));
    if !published() {
        return Ok(Some(false));
    }
    let settings = match changed_build_settings() {
        Ok(settings) => settings,
        Err(error) => {
            report(&format!("Could not plan what changed: {error}\n"));
            return Ok(Some(false));
        }
    };
    report("Planning what changed from the synced warehouse copy...\n");
    let Some(document) = plan_location(
        (options, runtime),
        (&BTreeSet::new(), &settings),
        (client, session_id, generation),
        stopped,
    )
    .await?
    else {
        return Ok(None);
    };
    if !published() {
        report("Project files changed while planning what changed; not reported.\n");
        return Ok(Some(false));
    }
    let (changed, downstream) = match changed_build_nodes(&document) {
        Ok(nodes) => nodes,
        Err(error) => {
            report(&format!("Could not plan what changed: {error}\n"));
            return Ok(Some(false));
        }
    };
    let counts = format!("{} changed, {} downstream", changed.len(), downstream.len());
    let request = ChangedBuildReport {
        session_id,
        generation,
        changed,
        downstream,
    };
    Ok(Some(
        match client.report_changed_build(&options.target, &request).await {
            Ok(_) => {
                report(&format!("Planned what changed: {counts}.\n"));
                true
            }
            Err(error) => {
                report(&format!("Reporting what changed failed: {error}\n"));
                false
            }
        },
    ))
}
