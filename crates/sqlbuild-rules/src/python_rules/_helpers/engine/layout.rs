//! SQLBuild's own layout rules for the `python/` package: what may sit at its root, and package names.

use std::collections::BTreeSet;

use crate::models::{Fault, PythonSource};
use crate::python_rules::_helpers::engine::faults::catalogue_fault;
use crate::python_rules::_helpers::engine::placement::package_anchor;
use crate::python_rules::constants::{
    GENERIC_PACKAGE_NAMES, INIT_FILE, NODE_KIND_DIRECTORIES, PACKAGE_ROOT,
    PROJECT_ROOT_ALLOWLIST_CODE, ROLE_DIRECTORIES, RUNTIME_PACKAGE_OWNER_NAMES_CODE,
    SHARED_UTILITIES_PACKAGE,
};

/// Faults of the selected layout rules among `codes`, one per offending entry or package.
pub(crate) fn layout_faults(sources: &[PythonSource], codes: &BTreeSet<&str>) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    if codes.contains(PROJECT_ROOT_ALLOWLIST_CODE) {
        faults.extend(root_entry_faults(sources));
    }
    if codes.contains(RUNTIME_PACKAGE_OWNER_NAMES_CODE) {
        faults.extend(generic_package_faults(sources));
    }
    faults
}

/// Direct modules, node-kind folders and role folders directly under `python/`.
fn root_entry_faults(sources: &[PythonSource]) -> Vec<Fault> {
    let mut reported: BTreeSet<String> = BTreeSet::new();
    let mut faults: Vec<Fault> = Vec::new();
    for parts in package_parts(sources) {
        let [entry, rest @ ..] = parts.as_slice() else {
            continue;
        };
        let folder: bool = !rest.is_empty();
        let kind: &str = if !folder {
            if *entry == INIT_FILE {
                continue;
            }
            "a module directly under python/"
        } else if NODE_KIND_DIRECTORIES.contains(entry) {
            "a node-kind folder"
        } else if ROLE_DIRECTORIES.iter().any(|(name, _)| name == entry) {
            "a role folder"
        } else {
            continue;
        };
        let path: String = format!("{PACKAGE_ROOT}/{entry}");
        if !reported.insert(path.clone()) {
            continue;
        }
        let (shown, reported): (String, Option<&str>) = if folder {
            (format!("{path}/"), package_anchor(sources, &path))
        } else {
            (path.clone(), Some(path.as_str()))
        };
        let detail: String = format!("{shown} is {kind}");
        faults.extend(
            reported.and_then(|anchor| layout_fault(PROJECT_ROOT_ALLOWLIST_CODE, anchor, &detail)),
        );
    }
    faults
}

/// Packages under `python/` with generic names, except `utils/` directly under it.
fn generic_package_faults(sources: &[PythonSource]) -> Vec<Fault> {
    let mut packages: BTreeSet<String> = BTreeSet::new();
    for parts in package_parts(sources) {
        let folders: &[&str] = &parts[..parts.len().saturating_sub(1)];
        for (depth, folder) in folders.iter().enumerate() {
            let allowed: bool = depth == 0 && *folder == SHARED_UTILITIES_PACKAGE;
            if GENERIC_PACKAGE_NAMES.contains(folder) && !allowed {
                packages.insert(format!("{PACKAGE_ROOT}/{}", folders[..=depth].join("/")));
            }
        }
    }
    packages
        .iter()
        .filter_map(|package| {
            let anchor: &str = package_anchor(sources, package)?;
            layout_fault(
                RUNTIME_PACKAGE_OWNER_NAMES_CODE,
                anchor,
                &format!("{package}/ has a generic name"),
            )
        })
        .collect()
}

/// Each `python/nodes/` file's path parts below `python/nodes/`.
fn package_parts(sources: &[PythonSource]) -> impl Iterator<Item = Vec<&str>> {
    sources.iter().filter_map(|source| {
        let below: &str = source
            .relative_path
            .strip_prefix(PACKAGE_ROOT)?
            .strip_prefix('/')?;
        Some(below.split('/').collect())
    })
}

/// A layout fault of rule `code` on the file at `path`.
fn layout_fault(code: &str, path: &str, detail: &str) -> Option<Fault> {
    catalogue_fault(code, path, (1, 1), detail)
}
