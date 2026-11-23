//! Where one module declares its nodes, and which declaration files share a unit folder.

use std::collections::BTreeMap;

use crate::node_files::constants::{
    CHECKS_FILE, KIND_FILES, MIXED_FACTORY_FILE, SINGLE_NODE_FILES,
};
use crate::node_files::models::{KindFiles, NodeModule};

/// The first file-name problem of one module, if any.
pub(crate) fn module_problem(module: &NodeModule) -> Option<String> {
    let path: &str = &module.relative_path;
    let file: &str = path.rsplit('/').next().unwrap_or(path);
    for (name, kind) in &module.direct {
        let expected: &str = files(kind).direct;
        if file != expected {
            return Some(format!(
                "@{kind} '{name}' is declared in {path}; an @{kind} is declared in {expected}"
            ));
        }
    }
    for (name, kinds) in &module.factories {
        let (expected, made): (&str, String) = match kinds.as_slice() {
            [] => continue,
            [kind] => (files(kind).factory, format!("only {}", files(kind).plural)),
            _ => (
                MIXED_FACTORY_FILE,
                kinds
                    .iter()
                    .map(|kind| files(kind).plural)
                    .collect::<Vec<&str>>()
                    .join(" and "),
            ),
        };
        if file != expected {
            let rule: String = if kinds.len() == 1 {
                format!("a factory that makes {made} is declared in {expected}")
            } else {
                format!("a factory that makes more than one kind is declared in {expected}")
            };
            return Some(format!("@factory '{name}' in {path} makes {made}; {rule}"));
        }
    }
    if SINGLE_NODE_FILES.contains(&file) && module.direct.len() > 1 {
        let names: Vec<&str> = module
            .direct
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        return Some(format!(
            "{path} declares {} nodes ({}); {file} declares exactly one, so give each its own \
             unit folder or use a factory",
            names.len(),
            names.join(", ")
        ));
    }
    None
}

/// Two node declaration files in one unit folder; `checks.py` may sit beside one.
pub(crate) fn unit_folder_problem(modules: &[NodeModule]) -> Option<String> {
    let mut folders: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for module in modules {
        if module.direct.is_empty() && module.factories.iter().all(|(_, kinds)| kinds.is_empty()) {
            continue;
        }
        let (folder, file) = module
            .relative_path
            .rsplit_once('/')
            .unwrap_or(("", module.relative_path.as_str()));
        if file != CHECKS_FILE {
            folders.entry(folder).or_default().push(file);
        }
    }
    folders
        .into_iter()
        .find(|(_, files)| files.len() > 1)
        .map(|(folder, files)| {
            format!(
                "{folder}/ holds {}; a unit folder holds one node declaration file, plus checks.py",
                files.join(" and ")
            )
        })
}

/// The files of `kind`; every kind discovery reports is in the table.
fn files(kind: &str) -> KindFiles {
    KIND_FILES
        .into_iter()
        .find(|files| files.kind == kind)
        .unwrap_or(KIND_FILES[0])
}
