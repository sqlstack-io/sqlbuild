//! A requested plan runs in the location's runtime interpreter on frozen node keys, from the synced copy.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use serde_json::{Value, json};
use sqlbuild_orchestration::platform::models::LocationWorkClient;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::warehouse_metadata::models::{PreviewCopy, PreviewCopyRead};
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
    let (argv, select) = match plan_arguments(options, settings)
        .and_then(|argv| Ok((argv, selected_names(selection)?)))
    {
        Ok(planned) => planned,
        Err(message) => return Ok(Some(plan_failure(&message))),
    };
    let consumption: bool = settings
        .get("changed")
        .is_some_and(|changed| !matches!(changed, Value::Null | Value::Bool(false)));
    let held: Option<HeldCopy> = held_copy((&options.target, generation), consumption);
    let read_consumption: bool = consumption || held.as_ref().is_some_and(|held| held.consumption);
    let copy = match read_copy(
        client,
        &options.target,
        PreviewCopyRead {
            session_id,
            generation,
            consumption: read_consumption,
            versioned: true,
            known_version: held.as_ref().map(|held| held.version.clone()),
        },
    )
    .await
    {
        Ok(copy) => copy,
        Err(error) => {
            return Ok(Some(plan_failure(&format!(
                "the synced warehouse copy is unavailable: {error}"
            ))));
        }
    };
    let planned: HeldCopy = match held {
        Some(held) if copy.unchanged => held,
        _ => hold_copy((&options.target, generation), (copy, read_consumption))?,
    };
    let request = json!({"plan": {
        "argv": argv,
        "select": select,
        "copy": planned.path.to_string_lossy(),
        "version": planned.version,
        "consumption": planned.consumption,
    }});
    run_plan(runtime, request, stopped).await
}

/// The copy file the location last read, the mirror version it holds and whether it has consumption.
#[derive(Clone)]
struct HeldCopy {
    target: String,
    generation: i64,
    version: String,
    consumption: bool,
    path: PathBuf,
}

/// The location's one held copy; the runtime holds the state built from it under the same version.
static HELD_COPY: Mutex<Option<HeldCopy>> = Mutex::new(None);

/// The held copy of this target and generation, when it also has consumption a plan needs.
fn held_copy((target, generation): (&str, i64), consumption: bool) -> Option<HeldCopy> {
    HELD_COPY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .filter(|held| {
            held.target == target
                && held.generation == generation
                && (held.consumption || !consumption)
        })
}

/// A versioned read; a server without mirror versions rejects the fields, so it is read unversioned.
async fn read_copy(
    client: &LocationWorkClient,
    target: &str,
    read: PreviewCopyRead,
) -> Result<PreviewCopy, String> {
    match client.preview_copy(target, &read).await {
        Ok(copy) => Ok(copy),
        Err(_) => client
            .preview_copy(
                target,
                &PreviewCopyRead {
                    versioned: false,
                    known_version: None,
                    ..read
                },
            )
            .await
            .map_err(|error| error.to_string()),
    }
}

/// Write `copy` as the held copy file, replacing the previous one; a copy without a version gets a
/// fresh one, so the runtime never reuses a state built from it.
fn hold_copy(
    (target, generation): (&str, i64),
    (copy, consumption): (PreviewCopy, bool),
) -> Result<HeldCopy, ControlError> {
    let path: PathBuf =
        std::env::temp_dir().join(format!("sqlbuild-plan-copy-{}.json", Uuid::now_v7()));
    let version: String = copy
        .version
        .clone()
        .unwrap_or_else(|| Uuid::now_v7().to_string());
    std::fs::write(
        &path,
        serde_json::to_vec(&copy).map_err(std::io::Error::other)?,
    )?;
    let held = HeldCopy {
        target: target.to_owned(),
        generation,
        version,
        consumption,
        path,
    };
    let previous: Option<HeldCopy> = HELD_COPY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .replace(held.clone());
    if let Some(previous) = previous {
        drop(std::fs::remove_file(previous.path));
    }
    Ok(held)
}

/// The `sqb plan` command line of a frozen request: JSON output and its flags; the frozen node names
/// travel beside it, so the runtime need not parse one `--select` per node.
fn plan_arguments(
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

/// The node names of a frozen selection, each selected as `--select <name>` would.
fn selected_names(selection: &BTreeSet<String>) -> Result<Vec<String>, String> {
    selection
        .iter()
        .map(|key| {
            key.split_once(':')
                .map(|(_, name)| name.to_owned())
                .ok_or_else(|| "the plan request names an invalid node".to_owned())
        })
        .collect()
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
