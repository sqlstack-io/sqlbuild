//! A requested plan runs as an ordinary `sqb plan --json` child on the server-frozen node keys.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::Path;
use std::process::Stdio;

use serde_json::{Value, json};
use sqlbuild_orchestration::platform::constants::PLAN_DOCUMENT_BYTES;
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::io::AsyncReadExt;
use tokio::process::Child;

use crate::argv::main::accepted_settings_arguments::accepted_settings_arguments;
use crate::native_commands::_helpers::execution::location_child;
use crate::native_commands::constants::PLAN_COMMAND;
use crate::native_commands::models::LocationServe;

/// The plan document, an `{"error": ...}` report when planning failed, or `None` when stopped first.
pub(super) async fn plan_location(
    (options, script): (&LocationServe, &Path),
    (selection, settings): (&BTreeSet<String>, &BTreeMap<String, Value>),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<Value>, ControlError> {
    let mut child = match plan_child((options, script), (selection, settings)) {
        Ok(child) => child?,
        Err(message) => return Ok(Some(failure(&message))),
    };
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ControlError::Invalid("plan output is unavailable".into()))?;
    let mut output: Vec<u8> = Vec::new();
    let limit = u64::try_from(PLAN_DOCUMENT_BYTES).unwrap_or(u64::MAX) + 1;
    tokio::pin!(stopped);
    let status = tokio::select! {
        result = &mut stopped => {
            child.kill().await?;
            result?;
            return Ok(None);
        },
        result = async {
            let mut reader = stdout.take(limit);
            reader.read_to_end(&mut output).await?;
            drop(reader);
            child.wait().await
        } => result?,
    };
    if output.len() > PLAN_DOCUMENT_BYTES {
        return Ok(Some(failure("the plan is too large to show")));
    }
    Ok(Some(match serde_json::from_slice::<Value>(&output) {
        Ok(document) if document.is_object() && matches!(status.code(), Some(0 | 1)) => document,
        _ => failure("sqb plan did not produce a plan; see the location's log"),
    }))
}

/// The started child, or why the frozen request cannot become plan arguments.
fn plan_child(
    (options, script): (&LocationServe, &Path),
    (selection, settings): (&BTreeSet<String>, &BTreeMap<String, Value>),
) -> Result<std::io::Result<Child>, String> {
    let flags = accepted_settings_arguments(PLAN_COMMAND, settings)?;
    let mut command = location_child(options, script);
    command
        .args([
            PLAN_COMMAND,
            "--target",
            &options.target,
            "--json",
            "--no-color",
        ])
        .args(flags);
    for key in selection {
        let (_, name) = key
            .split_once(':')
            .ok_or_else(|| "the plan request names an invalid node".to_owned())?;
        command.args(["--select", name]);
    }
    Ok(command
        .env_remove("SQLBUILD_LOCATION_TOKEN")
        .env_remove("SQLBUILD_RUN_TOKEN")
        .stdout(Stdio::piped())
        .spawn())
}

fn failure(message: &str) -> Value {
    json!({"error": {"code": "plan_failed", "message": message, "help": null}})
}
