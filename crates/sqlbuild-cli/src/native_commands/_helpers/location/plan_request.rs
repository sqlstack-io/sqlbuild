//! The `sqb plan` a location's runtime runs for a frozen request, and the document it answers with.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;

use serde_json::Value;
use sqlbuild_orchestration::runs::models::PinnedJob;
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::sync::{mpsc, oneshot};

use crate::argv::main::accepted_settings_arguments::accepted_settings_arguments;
use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::{
    plan_error, plan_failure, without_diffs,
};
use crate::native_commands::_helpers::location::plan_document::plan_document;
use crate::native_commands::constants::CHANGED_SETTING;
use crate::native_commands::constants::{PLAN_COMMAND, PLAN_NOT_PRODUCED};
use crate::native_commands::models::{LocationPlanRequest, LocationRuntimeRequest, LocationServe};
use crate::native_commands::types::LocationPlan;

/// The `sqb plan` command line of a frozen request; its node names travel beside it.
pub(crate) fn plan_arguments(
    options: &LocationServe,
    settings: &BTreeMap<String, Value>,
) -> Result<Vec<String>, String> {
    let mut argv: Vec<String> = vec![
        "--project-dir".into(),
        options.project_dir.to_string_lossy().into_owned(),
        PLAN_COMMAND.into(),
        "--target".into(),
        options.target.clone(),
        "--json".into(),
        "--no-color".into(),
    ];
    argv.extend(accepted_settings_arguments(PLAN_COMMAND, settings)?);
    Ok(argv)
}

/// Whether a plan shows what changed, so it reads the copy's consumption: `--changed`, or else a pinned job's `changed`.
pub(crate) fn plans_changes(
    settings: &BTreeMap<String, Value>,
    jobs: &BTreeMap<String, PinnedJob>,
) -> bool {
    let changed: Option<&Value> = settings.get(CHANGED_SETTING).or_else(|| {
        jobs.values()
            .find_map(|job| job.settings.get(CHANGED_SETTING))
    });
    changed.is_some_and(|changed| !matches!(changed, Value::Null | Value::Bool(false)))
}

/// Every setting a plan request's command gives, `settings` and `given` together, and [`plans_changes`].
pub(crate) fn given_and_changes(
    (settings, given): (&BTreeMap<String, Value>, &BTreeSet<String>),
    jobs: &BTreeMap<String, PinnedJob>,
) -> (BTreeSet<String>, bool) {
    let all: BTreeSet<String> = given.iter().chain(settings.keys()).cloned().collect();
    (all, plans_changes(settings, jobs))
}

/// The node names of a frozen selection, each selected as `--select <name>` would, then `job:<name>` for each pinned job.
pub(crate) fn selected_names(
    selection: &BTreeSet<String>,
    jobs: &BTreeMap<String, PinnedJob>,
) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = selection
        .iter()
        .map(|key| selected_name(key))
        .collect::<Result<_, _>>()?;
    names.extend(jobs.keys().map(|job| format!("job:{job}")));
    Ok(names)
}

/// The name of one frozen node key, such as `model:orders`.
fn selected_name(key: &str) -> Result<String, String> {
    match key.split_once(':') {
        Some((_, name)) => Ok(name.to_owned()),
        None => Err("the plan request names an invalid node".to_owned()),
    }
}

/// Ask the runtime for the plan; its output becomes the plan document, a failure the report it gave.
pub(crate) async fn run_plan(
    runtime: &mpsc::Sender<LocationRuntimeRequest>,
    request: Value,
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<LocationPlan>, ControlError> {
    let (reply, answer) = oneshot::channel();
    runtime
        .send(LocationRuntimeRequest::Plan(LocationPlanRequest {
            request,
            reply,
        }))
        .await
        .map_err(|_| ControlError::Invalid("location compiler stopped".into()))?;
    tokio::pin!(stopped);
    let answer = tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        answer = answer => answer,
    };
    let reply = match answer {
        Ok(Ok(reply)) => reply,
        Ok(Err(error)) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Location plan failed: {error}\n"),
            )]));
            return Ok(Some(without_diffs(plan_failure(PLAN_NOT_PRODUCED))));
        }
        Err(_) => return Ok(Some(without_diffs(plan_failure(PLAN_NOT_PRODUCED)))),
    };
    Ok(Some(
        match (reply["output"].as_str(), reply["exit_code"].as_i64()) {
            (Some(output), Some(code)) => plan_document(output.as_bytes(), matches!(code, 0 | 1)),
            _ if reply["failed"].is_object() => without_diffs(plan_error(&reply["failed"])),
            _ => without_diffs(plan_failure(PLAN_NOT_PRODUCED)),
        },
    ))
}
