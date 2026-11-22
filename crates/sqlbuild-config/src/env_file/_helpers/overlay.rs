//! The overlay and marker a load of the chosen env file gives every process.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;

use sqlbuild_core::process_environment::constants::ENV_FILE_MARKER_VARIABLE;
use sqlbuild_core::process_environment::main::environment_var_os::environment_var_os;
use sqlbuild_core::process_environment::main::marker_json::marker_json;
use sqlbuild_core::process_environment::models::{EnvFileMarker, EnvironmentOverlay};

use crate::env_file::_helpers::read::{read_pairs, shared_read_warning};
use crate::env_file::errors::EnvFileError;
use crate::env_file::models::EnvFileChoice;
use crate::env_file::types::ChosenPairs;

/// The file's pairs and its permissions warning; `None` pairs when loading is off or an unconfigured file is absent.
pub(crate) fn chosen_pairs(choice: &EnvFileChoice) -> Result<ChosenPairs, EnvFileError> {
    let Some(path) = &choice.path else {
        return Ok((None, None));
    };
    match read_pairs(path, choice)? {
        Some(pairs) => Ok((Some(pairs), shared_read_warning(path, choice))),
        None if choice.configured => Err(EnvFileError::Missing {
            choice: choice.clone(),
        }),
        None => Ok((None, None)),
    }
}

/// The variables a load takes and those the environment already set, sorted; `previous` names an earlier load's.
pub(crate) fn taken_variables(
    pairs: Vec<(String, String)>,
    previous: &[String],
) -> (
    Vec<(OsString, OsString)>,
    BTreeSet<String>,
    BTreeSet<String>,
) {
    let mut set: Vec<(OsString, OsString)> = Vec::new();
    let mut names: BTreeSet<String> = BTreeSet::new();
    let mut overridden: BTreeSet<String> = BTreeSet::new();
    for (name, value) in pairs {
        if name == ENV_FILE_MARKER_VARIABLE {
            continue;
        }
        let earlier: bool = previous.contains(&name);
        if !earlier && environment_var_os(&name).is_some() {
            overridden.insert(name);
            continue;
        }
        set.push((OsString::from(&name), OsString::from(value)));
        names.insert(name);
    }
    (set, names, overridden)
}

/// The overlay: the variables taken, an earlier load's variables this one drops removed, and the marker.
pub(crate) fn overlay_with_marker(
    set: Vec<(OsString, OsString)>,
    previous: Vec<String>,
    marker: &EnvFileMarker,
) -> EnvironmentOverlay {
    let mut overlay = EnvironmentOverlay {
        set,
        remove: previous
            .into_iter()
            .filter(|name| !marker.names.contains(name))
            .map(OsString::from)
            .collect(),
    };
    overlay.set.push((
        OsString::from(ENV_FILE_MARKER_VARIABLE),
        OsString::from(marker_json(marker)),
    ));
    overlay
}

/// The marker child processes inherit, so they don't load the same file again.
pub(crate) fn marker_for(
    (project_dir, target): (&Path, Option<&str>),
    choice: &EnvFileChoice,
    (loaded, names, overridden): (bool, BTreeSet<String>, BTreeSet<String>),
) -> EnvFileMarker {
    let canonical = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());
    EnvFileMarker {
        project_dir: canonical.to_string_lossy().into_owned(),
        target: target.map(str::to_owned),
        path: choice
            .path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        loaded,
        names: names.into_iter().collect(),
        overridden: overridden.into_iter().collect(),
    }
}
