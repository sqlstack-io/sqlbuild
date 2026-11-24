//! Where each project Python file sits, in the terms Fensu's rules read.

use crate::models::PythonSource;
use crate::python_rules::constants::{
    INIT_FILE, MAIN_ROLE, PACKAGE_ROOT, ROLE_DIRECTORIES, ROLE_FILES, ROOT_SCOPE, TEST_SCOPE,
    UNIT_MAIN_FILE,
};
use crate::python_rules::models::PythonTarget;

/// Place every source: files under `python/` in the runtime package, the rest under their own folder.
pub(crate) fn python_targets(sources: &[PythonSource]) -> Vec<PythonTarget> {
    sources.iter().enumerate().map(placed).collect()
}

/// Place every Python test under its own folder in Fensu's test scope.
pub(crate) fn test_targets(sources: &[PythonSource]) -> Vec<PythonTarget> {
    sources
        .iter()
        .enumerate()
        .map(|source| PythonTarget {
            scope: TEST_SCOPE,
            ..placed(source)
        })
        .collect()
}

fn placed((source, python): (usize, &PythonSource)) -> PythonTarget {
    let below: Option<&str> = python
        .relative_path
        .strip_prefix(PACKAGE_ROOT)
        .and_then(|rest| rest.strip_prefix('/'))
        .filter(|rest| !rest.is_empty());
    if let Some(below) = below {
        let relative_parts: Vec<String> = below.split('/').map(str::to_owned).collect();
        let first_role: Option<&str> = first_directory_role(&relative_parts);
        let unit_main: bool = first_role.is_none()
            && relative_parts.len() > 1
            && relative_parts
                .last()
                .is_some_and(|name| name == UNIT_MAIN_FILE);
        let is_main_module: bool = first_role == Some(MAIN_ROLE) || unit_main;
        return PythonTarget {
            source,
            repository_path: python.relative_path.clone(),
            package: true,
            root: PACKAGE_ROOT.to_owned(),
            role: role(&relative_parts),
            is_main_module,
            is_entry_module: is_main_module
                && relative_parts.last().is_some_and(|name| name != INIT_FILE),
            relative_parts,
            scope: ROOT_SCOPE,
        };
    }
    let (root, file) = python
        .relative_path
        .rsplit_once('/')
        .unwrap_or(("", python.relative_path.as_str()));
    PythonTarget {
        source,
        repository_path: python.relative_path.clone(),
        package: false,
        root: root.to_owned(),
        relative_parts: vec![file.to_owned()],
        role: None,
        is_main_module: false,
        is_entry_module: false,
        scope: ROOT_SCOPE,
    }
}

/// Fensu's `role_of`: a role file name wins, else the first role directory on the path.
fn role(relative_parts: &[String]) -> Option<String> {
    let file: &str = relative_parts.last()?;
    ROLE_FILES
        .iter()
        .find(|(name, _)| *name == file)
        .map(|(_, role)| *role)
        .or_else(|| first_directory_role(relative_parts))
        .map(str::to_owned)
}

fn first_directory_role(relative_parts: &[String]) -> Option<&'static str> {
    relative_parts[..relative_parts.len().saturating_sub(1)]
        .iter()
        .find_map(|part| directory_role(part))
}

fn directory_role(directory: &str) -> Option<&'static str> {
    ROLE_DIRECTORIES
        .iter()
        .find(|(name, _)| *name == directory)
        .map(|(_, role)| *role)
}

/// Fensu's anchor of a package folder: its `__init__.py`, else its first Python file in path order.
pub(crate) fn package_anchor<'a>(sources: &'a [PythonSource], package: &str) -> Option<&'a str> {
    let initializer: String = format!("{package}/{INIT_FILE}");
    let prefix: String = format!("{package}/");
    let mut paths = sources.iter().map(|source| source.relative_path.as_str());
    let first: Option<&str> = paths.clone().find(|path| path.starts_with(&prefix));
    paths.find(|path| *path == initializer).or(first)
}
