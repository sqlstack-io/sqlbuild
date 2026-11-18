//! The installed `sqlbuild` package Python would import, found without starting Python.

use std::path::{Path, PathBuf};

use crate::native_commands::constants::{
    PACKAGE_MARKER, PACKAGE_NAME, PATH_FILE_COMMENT, PATH_FILE_EXTENSION,
    PATH_FILE_IMPORT_PREFIXES, PYTHON_PATH_VARIABLES, SKILLS_DIRECTORY,
};
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::models::InstalledPackage;

/// The site directories of the installation whose scripts directory holds `script`.
#[cfg(not(windows))]
fn site_directories(prefix: &Path) -> Vec<PathBuf> {
    use crate::native_commands::constants::{
        INTERPRETER_DIRECTORY_PREFIX, LIBRARY_DIRECTORY, SITE_DIRECTORIES,
    };

    let Ok(entries) = std::fs::read_dir(prefix.join(LIBRARY_DIRECTORY)) else {
        return Vec::new();
    };
    let mut directories: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with(INTERPRETER_DIRECTORY_PREFIX)
        {
            continue;
        }
        for site in SITE_DIRECTORIES {
            let directory: PathBuf = entry.path().join(site);
            if directory.is_dir() {
                directories.push(directory);
            }
        }
    }
    directories.sort();
    directories
}

#[cfg(windows)]
fn site_directories(prefix: &Path) -> Vec<PathBuf> {
    use crate::native_commands::constants::WINDOWS_SITE_DIRECTORY;

    let directory: PathBuf = WINDOWS_SITE_DIRECTORY
        .iter()
        .fold(prefix.to_path_buf(), |path, part| path.join(part));
    if directory.is_dir() {
        vec![directory]
    } else {
        Vec::new()
    }
}

fn package_beneath(directory: &Path) -> Option<PathBuf> {
    let package: PathBuf = directory.join(PACKAGE_NAME);
    package.join(PACKAGE_MARKER).is_file().then_some(package)
}

/// The paths a site directory's `.pth` files add, in the order `site` reads them.
fn path_file_entries(site: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(site) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == PATH_FILE_EXTENSION)
        })
        .collect();
    files.sort();
    let mut added: Vec<PathBuf> = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for line in text.lines() {
            let line: &str = line.trim_end();
            if line.is_empty()
                || line.starts_with(PATH_FILE_COMMENT)
                || PATH_FILE_IMPORT_PREFIXES
                    .iter()
                    .any(|prefix| line.starts_with(prefix))
            {
                continue;
            }
            added.push(site.join(line));
        }
    }
    added
}

/// The one `sqlbuild` package the installation beside `script` holds, unless that is unclear.
pub(crate) fn installed_package(script: &Path) -> Result<InstalledPackage, PythonNeeded> {
    if PYTHON_PATH_VARIABLES
        .iter()
        .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
    {
        return Err(PythonNeeded::Installation);
    }
    let script: PathBuf = std::fs::canonicalize(script).map_err(|_| PythonNeeded::Installation)?;
    let prefix: &Path = script
        .parent()
        .and_then(Path::parent)
        .ok_or(PythonNeeded::Installation)?;
    let mut found: Vec<PathBuf> = Vec::new();
    for site in site_directories(prefix) {
        let package: Option<PathBuf> = package_beneath(&site).or_else(|| {
            path_file_entries(&site)
                .iter()
                .find_map(|directory| package_beneath(directory))
        });
        if let Some(package) = package {
            found.push(std::fs::canonicalize(&package).unwrap_or(package));
        }
    }
    found.dedup();
    let [package] = found.as_slice() else {
        return Err(PythonNeeded::Installation);
    };
    Ok(InstalledPackage {
        skills_root: SKILLS_DIRECTORY
            .iter()
            .fold(package.clone(), |path, part| path.join(part)),
    })
}
