//! The phases of the orchestration inventory: the folder layout, then each kind's files.

use sqlbuild_pysource::orchestration_modules::models::PythonDeclarationKind;

use crate::_helpers::reading::read_authored_text;
use crate::constants::{ORCHESTRATION_DIRECTORY, PYTHON_FILE_SUFFIX};
use crate::models::{DiscoveryFailure, FailureKind};
use crate::orchestration_files::_helpers::python_modules::module_declaration;
use crate::orchestration_files::_helpers::read_group_file::read_group_file;
use crate::orchestration_files::_helpers::read_job_file::read_job_file;
use crate::orchestration_files::constants::{
    BYTECODE_DIRECTORY, CALENDARS_DIRECTORY, GROUPS_DIRECTORY, HIDDEN_PREFIX, JOBS_DIRECTORY,
    ROLE_DIRECTORIES, SENSORS_DIRECTORY, TOML_FILE_SUFFIX, TOML_JOBS_HELP,
};
use crate::orchestration_files::errors::FileError;
use crate::orchestration_files::models::{
    FilePosition, GroupFile, JobFile, NamedModule, OrchestrationFailure, PythonModules,
};
use crate::tree::main::children::children;
use crate::tree::main::rglob::rglob;
use crate::tree::models::{ProjectTree, TreeEntry};

/// `D017` at a file and position, with optional help.
pub(crate) fn failure(path: &str, error: FileError) -> OrchestrationFailure {
    let FilePosition { line, column } = error.position;
    OrchestrationFailure::Declaration(DiscoveryFailure {
        kind: FailureKind::Orchestration,
        message: format!("{path}:{line}:{column}: {}", error.message),
        help: error.help,
    })
}

fn layout(path: &str, message: String, help: Option<String>) -> OrchestrationFailure {
    failure(
        path,
        FileError {
            message,
            position: FilePosition { line: 1, column: 1 },
            help,
        },
    )
}

fn ignored(entry: &TreeEntry) -> bool {
    entry.name.starts_with(HIDDEN_PREFIX) || entry.name == BYTECODE_DIRECTORY
}

fn read(tree: &ProjectTree, path: &str) -> Result<String, OrchestrationFailure> {
    if tree.is_undecodable(path) {
        return Err(layout(
            path,
            "path is not valid UTF-8; rename it so SQLBuild can read it".to_owned(),
            None,
        ));
    }
    read_authored_text(&tree.absolute(path)).map_err(|failure| OrchestrationFailure::Unreadable {
        relative_path: path.to_owned(),
        failure,
    })
}

/// Refuse a file directly in `orchestration/` or a folder no declaration kind owns.
pub(crate) fn check_top_level(tree: &ProjectTree) -> Result<(), OrchestrationFailure> {
    for (path, entry) in children(tree, ORCHESTRATION_DIRECTORY)? {
        if ignored(&entry) {
            continue;
        }
        if !entry.is_dir {
            return Err(layout(
                &path,
                "files cannot sit directly in orchestration/".to_owned(),
                Some(if entry.name.ends_with(PYTHON_FILE_SUFFIX) {
                    TOML_JOBS_HELP.to_owned()
                } else {
                    format!(
                        "move it into one of orchestration/{}/",
                        ROLE_DIRECTORIES.join("/, orchestration/")
                    )
                }),
            ));
        }
        if !ROLE_DIRECTORIES.contains(&entry.name.as_str()) {
            let holds_python: bool = !rglob(tree, &path, |found| {
                found.name.ends_with(PYTHON_FILE_SUFFIX)
            })?
            .is_empty();
            return Err(layout(
                &path,
                format!("unknown orchestration folder '{}'", entry.name),
                Some(if holds_python {
                    TOML_JOBS_HELP.to_owned()
                } else {
                    format!("use one of: {}", ROLE_DIRECTORIES.join(", "))
                }),
            ));
        }
    }
    Ok(())
}

/// The files below one role folder, refusing any without `suffix`.
fn role_files(
    tree: &ProjectTree,
    role: &str,
    suffix: &str,
) -> Result<Vec<String>, OrchestrationFailure> {
    let root: String = format!("{ORCHESTRATION_DIRECTORY}/{role}");
    let files: Vec<String> = rglob(tree, &root, |entry| !entry.is_dir)?;
    let mut kept: Vec<String> = Vec::new();
    for path in files {
        let hidden: bool = path
            .split('/')
            .any(|part| part.starts_with(HIDDEN_PREFIX) || part == BYTECODE_DIRECTORY);
        if hidden {
            continue;
        }
        if !path.ends_with(suffix) {
            let python_job: bool = path.ends_with(PYTHON_FILE_SUFFIX)
                && matches!(role, JOBS_DIRECTORY | GROUPS_DIRECTORY);
            return Err(layout(
                &path,
                format!("orchestration/{role}/ holds only {suffix} files"),
                Some(match role {
                    _ if python_job => TOML_JOBS_HELP.to_owned(),
                    JOBS_DIRECTORY | GROUPS_DIRECTORY => {
                        "jobs and job groups are TOML; calendars and sensors are Python in orchestration/calendars/ and orchestration/sensors/".to_owned()
                    }
                    _ => "jobs and job groups are TOML in orchestration/jobs/ and orchestration/job_groups/".to_owned(),
                }),
            ));
        }
        kept.push(path);
    }
    Ok(kept)
}

/// Every job file with its path.
pub(crate) fn read_jobs(
    tree: &ProjectTree,
) -> Result<Vec<(String, JobFile)>, OrchestrationFailure> {
    let mut jobs: Vec<(String, JobFile)> = Vec::new();
    for path in role_files(tree, JOBS_DIRECTORY, TOML_FILE_SUFFIX)? {
        let job: JobFile =
            read_job_file(&read(tree, &path)?).map_err(|error| failure(&path, error))?;
        jobs.push((path, job));
    }
    Ok(jobs)
}

/// Every job group file with its path.
pub(crate) fn read_groups(
    tree: &ProjectTree,
) -> Result<Vec<(String, GroupFile)>, OrchestrationFailure> {
    let mut groups: Vec<(String, GroupFile)> = Vec::new();
    for path in role_files(tree, GROUPS_DIRECTORY, TOML_FILE_SUFFIX)? {
        let group: GroupFile =
            read_group_file(&read(tree, &path)?).map_err(|error| failure(&path, error))?;
        groups.push((path, group));
    }
    Ok(groups)
}

/// Every calendar and sensor module, read without importing it.
pub(crate) fn read_python_modules(
    tree: &ProjectTree,
    python_version: (u8, u8),
) -> Result<PythonModules, OrchestrationFailure> {
    let mut found = PythonModules::default();
    for (role, kind) in [
        (CALENDARS_DIRECTORY, PythonDeclarationKind::Calendar),
        (SENSORS_DIRECTORY, PythonDeclarationKind::Sensor),
    ] {
        for path in role_files(tree, role, PYTHON_FILE_SUFFIX)? {
            let stem: &str = path
                .rsplit('/')
                .next()
                .and_then(|name| name.strip_suffix(PYTHON_FILE_SUFFIX))
                .unwrap_or_default();
            let declared: Option<NamedModule> =
                module_declaration(&read(tree, &path)?, stem, kind, python_version)
                    .map_err(|error| failure(&path, error))?;
            if let Some(mut module) = declared {
                module.path = path.clone();
                match kind {
                    PythonDeclarationKind::Calendar => found.calendars.push(module),
                    PythonDeclarationKind::Sensor => found.sensors.push(module),
                }
            }
            found.modules.push(path);
        }
    }
    Ok(found)
}
