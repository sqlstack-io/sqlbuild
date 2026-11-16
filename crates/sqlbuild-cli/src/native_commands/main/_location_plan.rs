//! A requested plan runs in the location's runtime interpreter on frozen node keys, from the synced copy.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::PathBuf;

use serde_json::{Value, json};
use sqlbuild_orchestration::platform::models::LocationWorkClient;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::warehouse_metadata::models::PreviewCopyRead;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::argv::main::accepted_settings_arguments::accepted_settings_arguments;
use crate::argv::models::Stream;
use crate::native_commands::_helpers::execution::{plan_error, plan_failure};
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::constants::{PLAN_COMMAND, PLAN_NOT_PRODUCED};
use crate::native_commands::main::_plan_document::plan_document;
use crate::native_commands::models::{LocationPlanRequest, LocationRuntimeRequest, LocationServe};

/// The plan document, an `{"error": ...}` report when planning failed, or `None` when stopped first.
pub(super) async fn plan_location(
    (options, runtime): (&LocationServe, &mpsc::Sender<LocationRuntimeRequest>),
    (selection, settings): (&BTreeSet<String>, &BTreeMap<String, Value>),
    (client, session_id, generation): (&LocationWorkClient, Uuid, i64),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<Value>, ControlError> {
    let argv = match plan_arguments(options, (selection, settings)) {
        Ok(argv) => argv,
        Err(message) => return Ok(Some(plan_failure(&message))),
    };
    let read = PreviewCopyRead {
        session_id,
        generation,
        consumption: settings
            .get("changed")
            .is_some_and(|changed| !matches!(changed, Value::Null | Value::Bool(false))),
        versioned: false,
        known_version: None,
    };
    let copy = match client.preview_copy(&options.target, &read).await {
        Ok(copy) => copy,
        Err(error) => {
            return Ok(Some(plan_failure(&format!(
                "the synced warehouse copy is unavailable: {error}"
            ))));
        }
    };
    let path: PathBuf =
        std::env::temp_dir().join(format!("sqlbuild-plan-copy-{}.json", Uuid::now_v7()));
    std::fs::write(
        &path,
        serde_json::to_vec(&copy).map_err(std::io::Error::other)?,
    )?;
    let request = json!({"plan": {
        "argv": argv,
        "copy": path.to_string_lossy(),
    }});
    let planned = run_plan(runtime, request, stopped).await;
    drop(std::fs::remove_file(&path));
    planned
}

/// The `sqb plan` command line of a frozen request: JSON output, its flags and its node names.
fn plan_arguments(
    options: &LocationServe,
    (selection, settings): (&BTreeSet<String>, &BTreeMap<String, Value>),
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
    for key in selection {
        let (_, name) = key
            .split_once(':')
            .ok_or_else(|| "the plan request names an invalid node".to_owned())?;
        argv.extend(["--select".to_owned(), name.to_owned()]);
    }
    Ok(argv)
}

async fn run_plan(
    runtime: &mpsc::Sender<LocationRuntimeRequest>,
    request: Value,
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<Value>, ControlError> {
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
            return Ok(Some(plan_failure(PLAN_NOT_PRODUCED)));
        }
        Err(_) => return Ok(Some(plan_failure(PLAN_NOT_PRODUCED))),
    };
    Ok(Some(
        match (reply["output"].as_str(), reply["exit_code"].as_i64()) {
            (Some(output), Some(code)) => plan_document(output.as_bytes(), matches!(code, 0 | 1)),
            _ if reply["failed"].is_object() => plan_error(&reply["failed"]),
            _ => plan_failure(PLAN_NOT_PRODUCED),
        },
    ))
}
