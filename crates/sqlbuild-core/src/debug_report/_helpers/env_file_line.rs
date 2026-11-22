//! The `sqb debug` line for the env file this process tree loaded.

use crate::debug_report::models::{DebugLine, DebugStatus};
use crate::process_environment::models::EnvFileMarker;

/// Which env file loaded, by path and variable count; never a value.
pub(crate) fn env_file_line(marker: &EnvFileMarker) -> DebugLine {
    let Some(path) = &marker.path else {
        return DebugLine::checked(
            "env file",
            "off",
            (DebugStatus::Skip, Some("env_file = \"\"")),
        );
    };
    if !marker.loaded {
        return DebugLine::checked(
            "env file",
            path,
            (DebugStatus::Ok, Some("not found, optional")),
        );
    }
    let count: usize = marker.names.len();
    let mut status: String = format!(
        "loaded, {count} variable{}",
        if count == 1 { "" } else { "s" }
    );
    if !marker.overridden.is_empty() {
        status.push_str(&format!(
            ", {} already set in the environment",
            marker.overridden.len()
        ));
    }
    DebugLine::checked("env file", path, (DebugStatus::Ok, Some(&status)))
}
