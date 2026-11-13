//! A signed-in `sqb build` asks its configured location to refresh the warehouse copy, best-effort.

use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use sqlbuild_config::project::main::load_control_config::load_control_config;
use sqlbuild_config::project::main::project_config_paths::project_config_paths;
use sqlbuild_orchestration::api::main::parse_origin::parse_origin;

use crate::person_login::_helpers::{credentials, transport};
use crate::person_login::constants::{
    BUILD_SYNC_SECONDS, CONTROL_CA_CERT_ENV, CONTROL_LOCATION_ENV, CONTROL_SERVER_ENV,
};

fn setting(name: &str, configured: Option<String>) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or(configured)
}

/// One stderr line about the sync, or `None` when no location is configured or nobody is signed in.
///
/// The request runs on its own thread, so a slow keyring, resolver or server cannot hold the build's
/// exit past the bound; an abandoned request ends with the process.
pub(crate) fn sync_after_build(project_dir: &Path, target: &str) -> Option<String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let (project_dir, target) = (project_dir.to_path_buf(), target.to_owned());
    drop(std::thread::spawn(move || {
        drop(sender.send(request_sync(&project_dir, &target)));
    }));
    match receiver.recv_timeout(Duration::from_secs(BUILD_SYNC_SECONDS)) {
        Ok(line) => line,
        Err(RecvTimeoutError::Timeout) => Some(format!(
            "Warehouse sync skipped: the request did not finish within {BUILD_SYNC_SECONDS} seconds.\n"
        )),
        Err(RecvTimeoutError::Disconnected) => None,
    }
}

fn request_sync(project_dir: &Path, target: &str) -> Option<String> {
    let configured = load_control_config(&project_config_paths(project_dir)).ok()?;
    let server = setting(CONTROL_SERVER_ENV, configured.server);
    let location = setting(CONTROL_LOCATION_ENV, configured.location);
    let (server, location) = match (server, location) {
        (Some(server), Some(location)) => (server, location),
        (None, None) => return None,
        _ => {
            return Some(
                "Warehouse sync skipped: set both [control] server and location, or neither.\n"
                    .into(),
            );
        }
    };
    let skipped = |error: std::io::Error| Some(format!("Warehouse sync skipped: {error}\n"));
    let origin = match parse_origin(&server, "CLI") {
        Ok(origin) => origin,
        Err(error) => return skipped(std::io::Error::other(error)),
    };
    let credential = match credentials::load(&origin) {
        Ok(Some(credential)) => credential,
        Ok(None) => return None,
        Err(error) => return skipped(error),
    };
    let ca = std::env::var_os(CONTROL_CA_CERT_ENV).map(PathBuf::from);
    let client = match transport::connect(&origin, ca.as_deref()) {
        Ok(client) => client,
        Err(error) => return skipped(error),
    };
    let request = async {
        let targets = transport::location_targets(&client, &credential).await?;
        let Some(found) = targets
            .iter()
            .find(|candidate| candidate.name == location && candidate.target == target)
        else {
            let named = targets.iter().any(|candidate| candidate.name == location);
            return Ok(if named {
                format!(
                    "Warehouse sync skipped: location '{location}' has no target '{target}' you can view on {origin}.\n"
                )
            } else {
                format!(
                    "Warehouse sync skipped: no location named '{location}' that you can view on {origin}.\n"
                )
            });
        };
        transport::warehouse_sync(
            &client,
            &credential,
            (found.location_id, target),
            "Requested after sqb build",
        )
        .await?;
        Ok::<_, std::io::Error>(format!(
            "Warehouse sync requested for location '{location}' target '{target}'.\n"
        ))
    };
    let outcome = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(std::io::Error::other)
        .and_then(|runtime| runtime.block_on(request));
    match outcome {
        Ok(line) => Some(line),
        Err(error) => skipped(error),
    }
}
