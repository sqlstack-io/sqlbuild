//! `pathlib.Path` construction and `Path.resolve()` over native paths.

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use sqlbuild_cache::digest::constants::{
    EXCLUDED_DIRECTORIES, EXCLUDED_ROOT_DIRECTORIES, PRESENCE_ONLY_FILE_SUFFIXES,
};
use sqlbuild_cache::project_snapshot::main::snapshot_project_files::snapshot_project_files;
use sqlbuild_cache::project_snapshot::models::{PathStamp, SnapshotRules};
use sqlbuild_core::git_revisions::main::head_commit::head_commit;
use sqlbuild_rules::project_tree::main::project_real_path::project_real_path;
use tokio::time::Instant;

use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::models::{LocationWatch, ProjectLocation};
use crate::replay::main::output_file_identities::output_file_identities;

/// Observe the same project inputs as compiler reuse, excluding redirected command output.
pub(crate) fn location_watch(root: &Path) -> LocationWatch {
    let rules = location_rules();
    LocationWatch {
        observed: location_inputs(root, &rules, &[]),
        rules,
        local_files: Vec::new(),
        attempted: None,
        changed_at: tokio::time::Instant::now(),
        retry_at: None,
    }
}

/// Coalesce source edits and retry deadlines without replacing the retained attempted input.
pub(crate) fn observe_location_inputs(
    root: &Path,
    mut watch: LocationWatch,
) -> (LocationWatch, bool) {
    let current = location_inputs(root, &watch.rules, &watch.local_files);
    if watch.observed != current {
        watch.observed = current;
        watch.changed_at = Instant::now();
    }
    if watch.attempted.as_ref() == Some(&watch.observed)
        && watch.retry_at.is_none_or(|retry| Instant::now() < retry)
    {
        return (watch, false);
    }
    if watch.attempted.is_some() && watch.changed_at.elapsed() < Duration::from_millis(250) {
        return (watch, false);
    }
    watch.attempted = Some(watch.observed.clone());
    (watch, true)
}

fn location_rules() -> SnapshotRules {
    SnapshotRules {
        excluded: EXCLUDED_DIRECTORIES
            .iter()
            .map(ToString::to_string)
            .collect(),
        excluded_root: EXCLUDED_ROOT_DIRECTORIES
            .iter()
            .map(ToString::to_string)
            .collect(),
        presence_suffixes: PRESENCE_ONLY_FILE_SUFFIXES
            .iter()
            .map(ToString::to_string)
            .collect(),
        output_files: output_file_identities(),
    }
}

/// Git commits matter to publication metadata even when no working-tree file changes.
pub(crate) fn location_inputs(
    root: &Path,
    rules: &SnapshotRules,
    local_files: &[PathBuf],
) -> (Vec<PathStamp>, Option<String>) {
    let mut stamps = snapshot_project_files(root, rules);
    stamps.retain(|stamp| {
        !rules
            .presence_suffixes
            .iter()
            .any(|suffix| stamp.relative_path.ends_with(suffix))
    });
    stamps = exclude_location_files(stamps, root, local_files);
    #[cfg(windows)]
    {
        let output: Vec<same_file::Handle> =
            [same_file::Handle::stdout(), same_file::Handle::stderr()]
                .into_iter()
                .flatten()
                .filter(|handle| {
                    handle
                        .as_file()
                        .metadata()
                        .is_ok_and(|metadata| metadata.is_file())
                })
                .collect();
        stamps.retain(|stamp| {
            same_file::Handle::from_path(root.join(&stamp.relative_path))
                .map_or(true, |handle| !output.contains(&handle))
        });
    }
    stamps.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    (stamps, head_commit(root))
}

/// Apply resolved exclusions to retained observations, never a fresh snapshot of possible source edits.
pub(crate) fn exclude_location_databases(
    root: &Path,
    files: Vec<PathBuf>,
    mut watch: LocationWatch,
) -> LocationWatch {
    watch.local_files.extend(files);
    watch.local_files.sort();
    watch.local_files.dedup();
    watch.observed.0 = exclude_location_files(watch.observed.0, root, &watch.local_files);
    watch.attempted = watch.attempted.map(|(stamps, commit)| {
        (
            exclude_location_files(stamps, root, &watch.local_files),
            commit,
        )
    });
    watch
}

fn exclude_location_files(
    mut stamps: Vec<PathStamp>,
    root: &Path,
    local_files: &[PathBuf],
) -> Vec<PathStamp> {
    stamps.retain(|stamp| !local_files.contains(&root.join(&stamp.relative_path)));
    stamps
}

pub(crate) fn location_database_files(root: &Path, files: Vec<String>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for file in files {
        let authored = pathlib_path(&root.join(file));
        let resolved = project_real_path(&authored);
        for path in [authored, resolved] {
            for suffix in ["", ".wal", "-wal", "-shm", "-journal"] {
                let mut file = path.as_os_str().to_os_string();
                file.push(suffix);
                paths.push(PathBuf::from(file));
            }
        }
    }
    paths
}

/// `Path(text)`: repeated separators, `.` parts and trailing separators dropped.
pub(crate) fn pathlib_path(text: &Path) -> PathBuf {
    let path: PathBuf = text
        .components()
        .filter(|component| *component != Component::CurDir)
        .collect();
    if path.as_os_str().is_empty() {
        return PathBuf::from(".");
    }
    path
}

/// `Path.resolve()`: absolute, with every existing prefix's links followed, as `realpath` does.
pub(crate) fn resolved_path(path: &Path, cwd: &Path) -> PathBuf {
    let absolute: PathBuf = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    if absolute.exists() {
        return project_real_path(&absolute);
    }
    let mut resolved: PathBuf = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => resolved.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(name) => {
                let candidate: PathBuf = resolved.join(name);
                resolved = if candidate.exists() {
                    project_real_path(&candidate)
                } else {
                    candidate
                };
            }
        }
    }
    resolved
}

/// The project directory as given and as `Path.resolve()` reads it.
pub(crate) fn project_location(given: Option<&Path>) -> Result<ProjectLocation, PythonNeeded> {
    let cwd: PathBuf = std::env::current_dir().map_err(|_| PythonNeeded::Installation)?;
    let given: PathBuf = match given {
        Some(raw) => pathlib_path(raw),
        None => cwd.clone(),
    };
    let resolved: PathBuf = resolved_path(&given, &cwd);
    if !spelled_as_given(&resolved, &cwd.join(&given)) {
        return Err(PythonNeeded::Installation);
    }
    Ok(ProjectLocation { given, resolved })
}

/// Links resolve as in Python on Unix; Windows drive substitutions and mappings stay Python's.
#[cfg(unix)]
fn spelled_as_given(_resolved: &Path, _absolute: &Path) -> bool {
    true
}

/// Whether the resolved path is the absolute path as given, apart from letter case.
#[cfg(not(unix))]
fn spelled_as_given(resolved: &Path, absolute: &Path) -> bool {
    let absolute: PathBuf = absolute
        .components()
        .filter(|component| *component != Component::CurDir)
        .collect();
    resolved
        .to_string_lossy()
        .eq_ignore_ascii_case(&absolute.to_string_lossy())
}
