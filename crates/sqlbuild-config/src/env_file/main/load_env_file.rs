//! Load the chosen env file under the environment's precedence.

use std::path::Path;

use sqlbuild_core::process_environment::main::env_file_marker::env_file_marker;

use crate::env_file::_helpers::choice::env_file_choice;
use crate::env_file::_helpers::overlay::{
    chosen_pairs, marker_for, overlay_with_marker, taken_variables,
};
use crate::env_file::errors::EnvFileError;
use crate::env_file::models::EnvFileLoad;

/// The overlay giving every process the env file's variables the environment leaves unset; `None` without a valid project.
pub fn load_env_file(
    project_dir: &Path,
    target: Option<&str>,
) -> Result<Option<EnvFileLoad>, EnvFileError> {
    let Some(choice) = env_file_choice(project_dir, target) else {
        return Ok(None);
    };
    let previous: Vec<String> = env_file_marker()
        .map(|marker| marker.names)
        .unwrap_or_default();
    let (pairs, warning) = chosen_pairs(&choice)?;
    let loaded: bool = pairs.is_some();
    let (set, names, overridden) = taken_variables(pairs.unwrap_or_default(), &previous);
    let marker = marker_for((project_dir, target), &choice, (loaded, names, overridden));
    let overlay = overlay_with_marker(set, previous, &marker);
    Ok(Some(EnvFileLoad {
        choice,
        overlay,
        marker,
        warning,
    }))
}
