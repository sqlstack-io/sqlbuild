//! `sqb dev` re-reads the project env file before each publication, so `r` picks up an edit.

use std::path::Path;

use sqlbuild_config::env_file::main::load_env_file::load_env_file;
use sqlbuild_core::process_environment::constants::ENV_FILE_MARKER_VARIABLE;
use sqlbuild_core::process_environment::main::env_file_marker::env_file_marker;
use sqlbuild_core::process_environment::main::environment_var_os::environment_var_os;
use sqlbuild_core::process_environment::main::install_environment_overlay::install_environment_overlay;
use sqlbuild_core::process_environment::models::EnvironmentOverlay;

use crate::native_commands::_helpers::location::execution::location_warning;
use crate::native_commands::models::LocationServe;

/// Between the parts of a ready card value.
const SEPARATOR: &str = " \u{b7} ";

/// Whether `overlay` changes any variable this process now sees, the marker aside.
fn changes_values(overlay: &EnvironmentOverlay) -> bool {
    overlay
        .set
        .iter()
        .filter(|(name, _)| name != ENV_FILE_MARKER_VARIABLE)
        .any(|(name, value)| environment_var_os(name).as_ref() != Some(value))
        || overlay
            .remove
            .iter()
            .any(|name| environment_var_os(name).is_some())
}

/// Re-read the env file under `sqb dev`, keeping the last good read with a warning; `true` means restart the interpreter.
pub(crate) fn reload_env_file(options: &LocationServe) -> bool {
    if !options.dev_events {
        return false;
    }
    match load_env_file(&options.project_dir, Some(&options.target)) {
        Ok(Some(load)) => {
            let changed: bool = changes_values(&load.overlay);
            if changed {
                install_environment_overlay(load.overlay);
            }
            changed
        }
        Ok(None) => false,
        Err(error) => {
            location_warning(
                "env file not reloaded",
                format!("{}; runs keep the values read before", error.message()),
            );
            false
        }
    }
}

/// The ready card's env file row: the file, how many variables it set, and how to reload it.
pub(crate) fn env_file_summary(project: &Path) -> Option<String> {
    let marker = env_file_marker()?;
    let Some(path) = marker.path else {
        return Some("off (env_file = \"\")".to_owned());
    };
    let root = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let shown: String = Path::new(&path).strip_prefix(&root).map_or_else(
        |_| path.clone(),
        |relative| relative.to_string_lossy().into_owned(),
    );
    if !marker.loaded {
        return Some(format!(
            "{shown} not found{SEPARATOR}create it, then press r to load it"
        ));
    }
    let count: usize = marker.names.len();
    let mut text: String = format!(
        "{shown}{SEPARATOR}{count} variable{}",
        if count == 1 { "" } else { "s" }
    );
    if !marker.overridden.is_empty() {
        text.push_str(&format!(
            " ({} already set in the environment)",
            marker.overridden.len()
        ));
    }
    text.push_str(&format!("{SEPARATOR}edit it, then press r to reload"));
    Some(text)
}
