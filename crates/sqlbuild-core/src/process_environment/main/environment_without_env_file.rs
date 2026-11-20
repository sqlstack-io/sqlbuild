//! This process's environment as it was before any env file loaded.

use std::ffi::OsString;

use crate::process_environment::constants::ENV_FILE_MARKER_VARIABLE;
use crate::process_environment::main::env_file_marker::env_file_marker;
use crate::process_environment::main::environment_vars_os::environment_vars_os;

/// The effective environment without the marker or any variable an env file set, so a child loads the file afresh.
#[must_use]
pub fn environment_without_env_file() -> Vec<(OsString, OsString)> {
    let taken: Vec<String> = env_file_marker()
        .map(|marker| marker.names)
        .unwrap_or_default();
    let mut variables: Vec<(OsString, OsString)> = Vec::new();
    for (name, value) in environment_vars_os() {
        let from_file: bool = name == ENV_FILE_MARKER_VARIABLE
            || name
                .to_str()
                .is_some_and(|name| taken.iter().any(|taken| taken == name));
        if !from_file {
            variables.push((name, value));
        }
    }
    variables
}
