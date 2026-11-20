//! Create the blank project tree as Python's `pathlib` calls did.

use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::Path;

use crate::constants::PROJECT_CONFIG_FILENAME;
use crate::init_command::constants::{GITIGNORE_FILE, GITKEEP_FILE, SCAFFOLD_DIRECTORIES};
use crate::init_command::errors::ScaffoldError;
use crate::init_command::main::ignore_env_file::ignore_env_file;

fn failed(error: std::io::Error, path: &Path) -> ScaffoldError {
    ScaffoldError {
        error,
        path: path.to_path_buf(),
    }
}

/// `Path.mkdir(parents=True, exist_ok=True)`.
fn make_directories(path: &Path) -> Result<(), ScaffoldError> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let Some(parent) = path.parent().filter(|parent| *parent != path) else {
                return Err(failed(error, path));
            };
            make_directories(parent)?;
            match fs::create_dir(path) {
                Err(error) if !path.is_dir() => Err(failed(error, path)),
                _ => Ok(()),
            }
        }
        Err(_) if path.is_dir() => Ok(()),
        Err(error) => Err(failed(error, path)),
    }
}

/// Create the resource directories, the project file, `.env` in `.gitignore`, and `.gitkeep` in empty directories.
pub fn scaffold_blank_project(base_dir: &Path, project_toml: &str) -> Result<(), ScaffoldError> {
    for directory in SCAFFOLD_DIRECTORIES {
        make_directories(&base_dir.join(directory))?;
    }
    let project_file = base_dir.join(PROJECT_CONFIG_FILENAME);
    fs::write(&project_file, project_toml).map_err(|error| failed(error, &project_file))?;
    ignore_env_file(base_dir).map_err(|error| failed(error, &base_dir.join(GITIGNORE_FILE)))?;
    for directory in SCAFFOLD_DIRECTORIES {
        let directory = base_dir.join(directory);
        let mut entries = fs::read_dir(&directory).map_err(|error| failed(error, &directory))?;
        if entries.next().is_none() {
            let gitkeep = directory.join(GITKEEP_FILE);
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&gitkeep)
                .map_err(|error| failed(error, &gitkeep))?;
        }
    }
    Ok(())
}
