//! Load the chosen env file under the environment's precedence.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;

use sqlbuild_core::process_environment::constants::ENV_FILE_MARKER_VARIABLE;
use sqlbuild_core::process_environment::main::env_file_marker::env_file_marker;
use sqlbuild_core::process_environment::main::environment_var_os::environment_var_os;
use sqlbuild_core::process_environment::main::marker_json::marker_json;
use sqlbuild_core::process_environment::models::{EnvFileMarker, EnvironmentOverlay};

use crate::env_file::_helpers::read::{read_pairs, shared_read_warning};
use crate::env_file::main::env_file_choice::env_file_choice;
use crate::env_file::models::{EnvFileError, EnvFileLoad};

/// The overlay that gives every process the env file's variables the environment leaves unset.
///
/// Variables an earlier load took from a file (named by the inherited marker) do not count as
/// set, so loading again replaces them: that is how `sqb dev` reloads the file. `None` without a
/// valid project configuration.
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
    let (pairs, warning): (Option<Vec<(String, String)>>, Option<String>) = match &choice.path {
        None => (None, None),
        Some(path) => match read_pairs(path, &choice)? {
            Some(pairs) => (Some(pairs), shared_read_warning(path, &choice)),
            None if choice.configured => return Err(EnvFileError::Missing { choice }),
            None => (None, None),
        },
    };
    let loaded: bool = pairs.is_some();
    let set_by_environment = |name: &str| {
        name == ENV_FILE_MARKER_VARIABLE
            || (!previous.iter().any(|taken| taken == name) && environment_var_os(name).is_some())
    };
    let mut overlay = EnvironmentOverlay::default();
    let mut names: BTreeSet<String> = BTreeSet::new();
    let mut overridden: BTreeSet<String> = BTreeSet::new();
    for (name, value) in pairs.unwrap_or_default() {
        if set_by_environment(&name) {
            if name != ENV_FILE_MARKER_VARIABLE {
                overridden.insert(name);
            }
            continue;
        }
        overlay
            .set
            .push((OsString::from(&name), OsString::from(value)));
        names.insert(name);
    }
    overlay.remove = previous
        .into_iter()
        .filter(|name| !names.contains(name))
        .map(OsString::from)
        .collect();
    let canonical = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());
    let marker = EnvFileMarker {
        project_dir: canonical.to_string_lossy().into_owned(),
        target: target.map(str::to_owned),
        path: choice
            .path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        loaded,
        names: names.into_iter().collect(),
        overridden: overridden.into_iter().collect(),
    };
    overlay.set.push((
        OsString::from(ENV_FILE_MARKER_VARIABLE),
        OsString::from(marker_json(&marker)),
    ));
    Ok(Some(EnvFileLoad {
        choice,
        overlay,
        marker,
        warning,
    }))
}
