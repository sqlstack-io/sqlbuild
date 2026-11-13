//! The post-build sync's phases: read the configured location, sign in, and ask for the sync.

use std::env::VarError;
use std::path::{Path, PathBuf};

use sqlbuild_config::project::main::load_control_config::load_control_config;
use sqlbuild_config::project::main::project_config_paths::project_config_paths;
use sqlbuild_orchestration::api::main::parse_origin::parse_origin;

use crate::person_login::_helpers::{credentials, transport};
use crate::person_login::constants::{
    CONTROL_CA_CERT_ENV, CONTROL_LOCATION_ENV, CONTROL_SERVER_ENV,
};
use crate::person_login::models::{PersonClient, PersonCredential};

/// The configured server and location name, from the environment or `[control]`; `None` when neither is set.
pub(crate) fn configured_location(
    project_dir: &Path,
) -> Result<Option<(String, String)>, std::io::Error> {
    let configured = load_control_config(&project_config_paths(project_dir))
        .map_err(|_| std::io::Error::other("sqlbuild_project.toml [control] could not be read."))?;
    match (
        setting(CONTROL_SERVER_ENV, configured.server)?,
        setting(CONTROL_LOCATION_ENV, configured.location)?,
    ) {
        (Some(server), Some(location)) => Ok(Some((server, location))),
        (None, None) => Ok(None),
        _ => Err(std::io::Error::other(
            "set both [control] server and location, or neither.",
        )),
    }
}

fn setting(name: &str, configured: Option<String>) -> Result<Option<String>, std::io::Error> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) | Err(VarError::NotPresent) => Ok(configured),
        Err(VarError::NotUnicode(_)) => Err(std::io::Error::other(format!("{name} must be text."))),
    }
}

/// Signs in with the stored session and asks for the sync; `None` when nobody is signed in to the server.
pub(crate) fn sign_in_and_sync(
    (server, location): (String, String),
    target: &str,
) -> Result<Option<String>, std::io::Error> {
    let origin = parse_origin(&server, "CLI").map_err(std::io::Error::other)?;
    let Some(credential) = credentials::load(&origin)? else {
        return Ok(None);
    };
    let ca = std::env::var_os(CONTROL_CA_CERT_ENV).map(PathBuf::from);
    let client = transport::connect(&origin, ca.as_deref())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime
        .block_on(sync_location_target(
            (&client, &credential),
            (&location, target),
            &origin,
        ))
        .map(Some)
}

async fn sync_location_target(
    (client, credential): (&PersonClient, &PersonCredential),
    (location, target): (&str, &str),
    origin: &str,
) -> Result<String, std::io::Error> {
    let targets = transport::location_targets(client, credential).await?;
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
        client,
        credential,
        (found.location_id, target),
        "Requested after sqb build",
    )
    .await?;
    Ok(format!(
        "Warehouse sync requested for location '{location}' target '{target}'.\n"
    ))
}
