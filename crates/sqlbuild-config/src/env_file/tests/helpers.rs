use std::ffi::OsString;
use std::path::Path;

use sqlbuild_core::process_environment::constants::ENV_FILE_MARKER_VARIABLE;
use sqlbuild_core::process_environment::main::marker_json::marker_json;
use sqlbuild_core::process_environment::models::{EnvFileMarker, EnvironmentOverlay};
use tempfile::TempDir;

use crate::env_file::models::EnvFileLoad;

/// A project with `toml` after its `name` and `adapter`, and `files` readable by their owner only.
pub(super) fn project(toml: &str, files: &[(&str, &str)]) -> TempDir {
    let directory = TempDir::new().expect("temporary directory");
    std::fs::write(
        directory.path().join("sqlbuild_project.toml"),
        format!("name = \"shop\"\nadapter = \"duckdb\"\n{toml}"),
    )
    .expect("project file");
    for (path, contents) in files {
        let path = directory.path().join(path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("directory");
        std::fs::write(&path, contents).expect("env file");
        with_mode(&path, 0o600);
    }
    directory
}

/// Set a file's permission bits; other platforms have none to set.
#[cfg(unix)]
pub(super) fn with_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
}

/// Set a file's permission bits; other platforms have none to set.
#[cfg(not(unix))]
pub(super) fn with_mode(_path: &Path, _mode: u32) {}

/// The variables a load sets for every process, the marker aside.
pub(super) fn set_pairs(load: &EnvFileLoad) -> Vec<(String, String)> {
    load.overlay
        .set
        .iter()
        .filter(|(name, _)| name != ENV_FILE_MARKER_VARIABLE)
        .map(|(name, value)| {
            (
                name.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect()
}

/// The environment an earlier load of `project_dir`'s `.env` left: `earlier` set, and the marker naming them.
pub(super) fn earlier_load(project_dir: &Path, earlier: &[(&str, &str)]) -> EnvironmentOverlay {
    let mut names: Vec<String> = earlier.iter().map(|(name, _)| (*name).to_owned()).collect();
    names.sort();
    let marker = EnvFileMarker {
        project_dir: project_dir
            .canonicalize()
            .expect("project directory")
            .to_string_lossy()
            .into_owned(),
        target: None,
        path: Some(project_dir.join(".env").to_string_lossy().into_owned()),
        loaded: true,
        names,
        overridden: Vec::new(),
    };
    let mut set: Vec<(OsString, OsString)> = earlier
        .iter()
        .map(|(name, value)| (OsString::from(name), OsString::from(value)))
        .collect();
    set.push((
        OsString::from(ENV_FILE_MARKER_VARIABLE),
        OsString::from(marker_json(&marker)),
    ));
    EnvironmentOverlay {
        set,
        remove: Vec::new(),
    }
}
