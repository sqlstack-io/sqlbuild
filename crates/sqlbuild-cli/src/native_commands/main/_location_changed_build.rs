//! After each publish and each completed sync the location plans the web UI's "Build what changed" with the plan page's own path, from the synced copy, and reports what it runs.

use std::collections::BTreeSet;
use std::future::Future;

use sqlbuild_orchestration::node_states::constants::MAX_FAILING_REASON_CHARS;
use sqlbuild_orchestration::node_states::models::ChangedBuildReport;
use sqlbuild_orchestration::platform::models::LocationWorkClient;
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::paths::location_files;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::changed_build::{
    changed_build_nodes, changed_build_settings,
};
use crate::native_commands::main::_location_plan::plan_location;
use crate::native_commands::models::{LocationRuntimeRequest, LocationServe, LocationWatch};

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
    let Some((document, _)) = plan_location(
        (options, runtime),
        (&BTreeSet::new(), &settings, false),
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
    if let Some(reason) = document["error"]["message"].as_str() {
        report(&format!("Could not plan what changed: {reason}\n"));
        let request = ChangedBuildReport {
            session_id,
            generation,
            changed: Vec::new(),
            downstream: Vec::new(),
            failing: Some(reason.chars().take(MAX_FAILING_REASON_CHARS).collect()),
        };
        let reported = match client.report_changed_build(&options.target, &request).await {
            Ok(_) => document["error"]["code"].as_str() != Some("plan_failed"),
            Err(error) => {
                report(&format!("Reporting the failed plan failed: {error}\n"));
                false
            }
        };
        return Ok(Some(reported));
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
        failing: None,
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
