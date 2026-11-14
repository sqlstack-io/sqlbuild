//! Discover, read and check every orchestration declaration without importing project Python.

use sqlbuild_pysource::orchestration_modules::models::PythonDeclarationKind;

use crate::_helpers::reading::read_authored_text;
use crate::constants::{ORCHESTRATION_DIRECTORY, PYTHON_FILE_SUFFIX};
use crate::models::{DiscoveryFailure, FailureKind};
use crate::orchestration_files::_helpers::python_modules::module_declaration;
use crate::orchestration_files::_helpers::references::{check_references, check_renames};
use crate::orchestration_files::constants::{
    BYTECODE_DIRECTORY, CALENDARS_DIRECTORY, GROUPS_DIRECTORY, HIDDEN_PREFIX, JOBS_DIRECTORY,
    RENAMES_FILE, SENSORS_DIRECTORY, TOML_FILE_SUFFIX,
};
use crate::orchestration_files::main::read_group_file::read_group_file;
use crate::orchestration_files::main::read_job_file::read_job_file;
use crate::orchestration_files::main::read_renames_file::read_renames_file;
use crate::orchestration_files::models::{
    FileError, FilePosition, OrchestrationFailure, OrchestrationInventory,
};
use crate::tree::main::children::children;
use crate::tree::main::rglob::rglob;
use crate::tree::models::{ProjectTree, TreeEntry};

const ROLE_DIRECTORIES: [&str; 4] = [
    JOBS_DIRECTORY,
    GROUPS_DIRECTORY,
    CALENDARS_DIRECTORY,
    SENSORS_DIRECTORY,
];

/// `D017` at a file and position, with optional help.
fn failure(path: &str, error: FileError) -> OrchestrationFailure {
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
            return Err(layout(
                &path,
                format!("orchestration/{role}/ holds only {suffix} files"),
                Some(match role {
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

/// Every job, group, calendar and sensor below `orchestration/`, or the first problem with them.
pub fn orchestration_inventory(
    tree: &ProjectTree,
    python_version: (u8, u8),
) -> Result<OrchestrationInventory, OrchestrationFailure> {
    let mut inventory = OrchestrationInventory::default();
    if !tree.is_dir(ORCHESTRATION_DIRECTORY) {
        return Ok(inventory);
    }
    for (path, entry) in children(tree, ORCHESTRATION_DIRECTORY)? {
        if ignored(&entry) {
            continue;
        }
        if !entry.is_dir && entry.name == RENAMES_FILE {
            let renames =
                read_renames_file(&read(tree, &path)?).map_err(|error| failure(&path, error))?;
            inventory.renames = Some((path, renames));
            continue;
        }
        if !entry.is_dir {
            return Err(layout(
                &path,
                format!("files cannot sit directly in orchestration/, except {RENAMES_FILE}"),
                Some(format!(
                    "move it into one of orchestration/{}/",
                    ROLE_DIRECTORIES.join("/, orchestration/")
                )),
            ));
        }
        if !ROLE_DIRECTORIES.contains(&entry.name.as_str()) {
            return Err(layout(
                &path,
                format!("unknown orchestration folder '{}'", entry.name),
                Some(format!("use one of: {}", ROLE_DIRECTORIES.join(", "))),
            ));
        }
    }
    for path in role_files(tree, JOBS_DIRECTORY, TOML_FILE_SUFFIX)? {
        let job = read_job_file(&read(tree, &path)?).map_err(|error| failure(&path, error))?;
        inventory.jobs.push((path, job));
    }
    for path in role_files(tree, GROUPS_DIRECTORY, TOML_FILE_SUFFIX)? {
        let group = read_group_file(&read(tree, &path)?).map_err(|error| failure(&path, error))?;
        inventory.groups.push((path, group));
    }
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
            let declared = module_declaration(&read(tree, &path)?, stem, kind, python_version)
                .map_err(|error| failure(&path, error))?;
            if let Some(mut module) = declared {
                module.path = path.clone();
                match kind {
                    PythonDeclarationKind::Calendar => inventory.calendars.push(module),
                    PythonDeclarationKind::Sensor => inventory.sensors.push(module),
                }
            }
            inventory.modules.push(path);
        }
    }
    check_references(&inventory).map_err(|(path, error)| failure(&path, error))?;
    check_renames(&inventory).map_err(|(path, error)| failure(&path, error))?;
    Ok(inventory)
}
