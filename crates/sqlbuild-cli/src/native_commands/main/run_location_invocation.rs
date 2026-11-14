//! A location supervisor stays native while each compilation uses the installed project interpreter; its health check needs no interpreter.

use std::path::Path;

use chrono::Utc;
use serde_json::json;
use sqlbuild_orchestration::platform::main::connect_location_work::connect_location_work;
use sqlbuild_orchestration::platform::models::LocationState;
use uuid::Uuid;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::decision::text;
use crate::native_commands::_helpers::execution::location_options;
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::main::_run_dev::run_dev;
use crate::native_commands::main::_run_location::run_location;

/// Health needs no Python CLI; serving and development return `None` without one so the caller reports it.
pub(crate) fn run_location_invocation(namespace: &Namespace, script: Option<&Path>) -> Option<i32> {
    let development = text(namespace, "command").as_deref() == Some("dev");
    let location_command = text(namespace, "location_command");
    if !development && text(namespace, "command").as_deref() != Some("location") {
        return None;
    }
    if !development && location_command.as_deref() == Some("health") {
        return Some(location_health(namespace));
    }
    if !development && location_command.as_deref() != Some("serve") {
        return None;
    }
    let script = script?;
    let result = (|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        if development {
            return runtime.block_on(run_dev(namespace));
        }
        let options = location_options(namespace)?;
        let bearer = std::env::var("SQLBUILD_LOCATION_TOKEN").map_err(|_| {
            std::io::Error::other(
                "SQLBUILD_LOCATION_TOKEN must contain a UTF-8 location credential",
            )
        })?;
        if bearer.trim().is_empty() {
            return Err(std::io::Error::other("SQLBUILD_LOCATION_TOKEN must not be blank").into());
        }
        runtime.block_on(run_location(options, script, &bearer))
    })();
    Some(match result {
        Ok(()) => {
            drop(write_output(&[(
                Stream::Stderr,
                if development {
                    "Development services stopped successfully\n"
                } else {
                    "Code location stopped successfully\n"
                },
            )]));
            0
        }
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!(
                    "{} failed: {error}\n",
                    if development {
                        "Development services"
                    } else {
                        "Code location"
                    }
                ),
            )]));
            1
        }
    })
}

/// `sqb location health`: 0 when healthy or degraded, 1 when down or falling back, 2 when unanswered.
fn location_health(namespace: &Namespace) -> i32 {
    drop(write_output(&[(
        Stream::Stderr,
        "Checking location health...\n",
    )]));
    match read_state(namespace) {
        Ok((location_id, target, state)) => {
            let document = json!({"location_id": location_id, "target": target, "state": state});
            drop(write_output(&[
                (Stream::Stdout, &format!("{document}\n")),
                (Stream::Stderr, "Location health checked successfully\n"),
            ]));
            i32::from(matches!(
                state,
                LocationState::Broken | LocationState::FallingBack
            ))
        }
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Location health check failed: {error}\n"),
            )]));
            2
        }
    }
}

fn read_state(namespace: &Namespace) -> Result<(Uuid, String, LocationState), String> {
    let location_id: Uuid = text(namespace, "location_id")
        .unwrap_or_default()
        .parse()
        .map_err(|_| "location ID must be a UUID".to_owned())?;
    let target = text(namespace, "target").unwrap_or_default();
    if target.trim().is_empty() {
        return Err("target must not be blank".into());
    }
    let bearer = match std::env::var("SQLBUILD_LOCATION_TOKEN") {
        Ok(bearer) if !bearer.trim().is_empty() => bearer,
        _ => return Err("SQLBUILD_LOCATION_TOKEN must contain the location's credential".into()),
    };
    let server_url = text(namespace, "location_server_url").unwrap_or_default();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    let observations = runtime
        .block_on(async {
            connect_location_work((&server_url, &bearer), location_id)?
                .recent_health(&target)
                .await
        })
        .map_err(|error| error.to_string())?;
    let state = LocationState::of(&observations, (&target, Utc::now()));
    Ok((location_id, target, state))
}
