//! The `SQLBUILD_ENV_FILE_LOADED` value of a marker.

use serde_json::{Value, json};

use crate::process_environment::models::EnvFileMarker;

/// The marker as the compact JSON object children read back with `env_file_marker`.
#[must_use]
pub fn marker_json(marker: &EnvFileMarker) -> String {
    let value: Value = json!({
        "project_dir": marker.project_dir,
        "target": marker.target,
        "path": marker.path,
        "loaded": marker.loaded,
        "names": marker.names,
        "overridden": marker.overridden,
    });
    value.to_string()
}
