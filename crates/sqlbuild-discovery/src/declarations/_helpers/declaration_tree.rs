//! Scan macro, enum and constant declaration files with their scope facts.

use crate::constants::{
    CANONICAL_AUTHORED_ROOTS, DECLARATION_GROUP_DIRECTORY, GLOBAL_DECLARATION_DIRECTORIES,
    GLOBAL_ROLE_PARENT, LOCAL_DECLARATION_DIRECTORIES, PYTHON_FILE_SUFFIX, PYTHON_MACROS_DIRECTORY,
    SCOPED_DECLARATION_DIRECTORIES, SQL_FILE_SUFFIX, SQL_MACROS_DIRECTORY,
};
use crate::declarations::_helpers::declaration_groups::validate_declaration_groups;
use crate::declarations::_helpers::paths::{directory_facts, join, name, parent, stem};
use crate::declarations::_helpers::scan::{declaration_failure, shown};
use crate::declarations::errors::ScanError;
use crate::declarations::models::{DeclarationFileFact, DeclarationKind, ScopeKind};
use crate::tree::main::directories::directories;
use crate::tree::main::path_order::compare_posix_text;
use crate::tree::main::rglob::rglob;
use crate::tree::models::ProjectTree;
use sqlbuild_core::constants::PYTHON_INIT_MODULE_STEM;

/// The SQL test folder of macro tests, which keeps Python macro files without a kind folder.
const MACRO_TESTS_ROOT: &str = "tests/unit/macros";

/// Where a declaration root sits and who owns it.
struct DeclarationRoot<'a> {
    ownership_root: &'a str,
    directory: &'a str,
    owning_path: Option<&'a str>,
    scope_kind: ScopeKind,
}

/// Every declaration file below every macro, enum and constant root, by relative path.
pub(crate) fn declaration_file_facts(
    tree: &ProjectTree,
    kind: Option<DeclarationKind>,
) -> Result<Vec<DeclarationFileFact>, ScanError> {
    validate_declaration_groups(tree)?;
    let mut local: Vec<&str> = LOCAL_DECLARATION_DIRECTORIES.to_vec();
    local.sort_unstable();
    if let Some(directory) = local
        .into_iter()
        .find(|directory| is_kind(kind, directory) && tree.is_dir(directory))
    {
        return Err(declaration_failure(format!(
            "Scoped declaration root {directory}/ must be below a canonical authored root"
        )));
    }
    let mut facts: Vec<DeclarationFileFact> = Vec::new();
    let mut global: Vec<&str> = GLOBAL_DECLARATION_DIRECTORIES.to_vec();
    global.sort_unstable();
    for name in global {
        let directory: String = join(GLOBAL_ROLE_PARENT, name);
        if is_kind(kind, name) && tree.is_dir(&directory) {
            facts.extend(files_under_root(
                tree,
                &DeclarationRoot {
                    ownership_root: &directory,
                    directory: &directory,
                    owning_path: None,
                    scope_kind: ScopeKind::Global,
                },
            )?);
        }
    }
    for root_parts in CANONICAL_AUTHORED_ROOTS {
        let root: String = root_parts.join("/");
        if tree.is_dir(&root) {
            facts.extend(scoped_root_files(tree, &root, root_parts.len(), kind)?);
        }
    }
    facts.sort_by(|left, right| compare_posix_text(&left.relative_path, &right.relative_path));
    Ok(facts)
}

fn scoped_root_files(
    tree: &ProjectTree,
    root: &str,
    root_length: usize,
    kind: Option<DeclarationKind>,
) -> Result<Vec<DeclarationFileFact>, ScanError> {
    let mut facts: Vec<DeclarationFileFact> = Vec::new();
    for directory in directories(tree, root)? {
        let Some((_, scope_kind)) = directory_facts(name(&directory)) else {
            continue;
        };
        if !is_kind(kind, name(&directory)) {
            continue;
        }
        let parts: Vec<&str> = directory.split('/').collect();
        let between: &[&str] = &parts[root_length..parts.len() - 1];
        if between
            .iter()
            .any(|part| SCOPED_DECLARATION_DIRECTORIES.contains(part))
        {
            return Err(nested_root_failure(&directory));
        }
        let mut owning_path: &str = parent(&directory);
        if name(owning_path) == DECLARATION_GROUP_DIRECTORY {
            owning_path = parent(owning_path);
        }
        facts.extend(files_under_root(
            tree,
            &DeclarationRoot {
                ownership_root: root,
                directory: &directory,
                owning_path: Some(owning_path),
                scope_kind,
            },
        )?);
    }
    Ok(facts)
}

/// Whether a declaration directory holds `kind`, or every kind when none is requested.
fn is_kind(kind: Option<DeclarationKind>, directory_name: &str) -> bool {
    match (kind, directory_facts(directory_name)) {
        (None, _) => true,
        (Some(kind), Some((directory_kind, _))) => directory_kind == kind,
        (Some(_), None) => false,
    }
}

fn nested_root_failure(directory: &str) -> ScanError {
    declaration_failure(format!(
        "Declaration root {}/ is nested inside another declaration tree",
        shown(directory)
    ))
}

fn files_under_root(
    tree: &ProjectTree,
    root: &DeclarationRoot<'_>,
) -> Result<Vec<DeclarationFileFact>, ScanError> {
    if let Some(nested) = directories(tree, root.directory)?
        .into_iter()
        .find(|directory| SCOPED_DECLARATION_DIRECTORIES.contains(&name(directory)))
    {
        return Err(nested_root_failure(&nested));
    }
    let Some((kind, _)) = directory_facts(name(root.directory)) else {
        return Ok(Vec::new());
    };
    if kind == DeclarationKind::Macro && root.directory != MACRO_TESTS_ROOT {
        return macro_files(tree, root);
    }
    let suffix: &str = if kind == DeclarationKind::Macro {
        PYTHON_FILE_SUFFIX
    } else {
        SQL_FILE_SUFFIX
    };
    Ok(
        rglob(tree, root.directory, |entry| entry.name.ends_with(suffix))?
            .into_iter()
            .filter(|path| {
                kind != DeclarationKind::Macro || stem(name(path)) != PYTHON_INIT_MODULE_STEM
            })
            .map(|relative_path| DeclarationFileFact {
                relative_path,
                kind,
                scope_kind: root.scope_kind,
                ownership_root: root.ownership_root.to_owned(),
                owning_path: root.owning_path.map(str::to_owned),
                declaration_root: root.directory.to_owned(),
            })
            .collect(),
    )
}

/// Python macros below `python/` and SQL macros below `sql/`; any other macro file is misplaced.
fn macro_files(
    tree: &ProjectTree,
    root: &DeclarationRoot<'_>,
) -> Result<Vec<DeclarationFileFact>, ScanError> {
    let paths: Vec<String> = rglob(tree, root.directory, |entry| {
        entry.name.ends_with(PYTHON_FILE_SUFFIX) || entry.name.ends_with(SQL_FILE_SUFFIX)
    })?;
    let mut facts: Vec<DeclarationFileFact> = Vec::new();
    for relative_path in paths {
        let inside: &str = &relative_path[root.directory.len() + 1..];
        let python: bool = relative_path.ends_with(PYTHON_FILE_SUFFIX);
        if python && stem(name(&relative_path)) == PYTHON_INIT_MODULE_STEM {
            continue;
        }
        let (folder, rest) = inside.split_once('/').unwrap_or(("", inside));
        let expected: &str = if python {
            PYTHON_MACROS_DIRECTORY
        } else {
            SQL_MACROS_DIRECTORY
        };
        if folder != expected {
            let destination: &str =
                if folder == PYTHON_MACROS_DIRECTORY || folder == SQL_MACROS_DIRECTORY {
                    rest
                } else {
                    inside
                };
            return Err(ScanError::Failure(crate::models::DiscoveryFailure {
                kind: crate::models::FailureKind::Declaration,
                message: format!(
                    "macro file {} must live in {}/{PYTHON_MACROS_DIRECTORY}/ or {}/{SQL_MACROS_DIRECTORY}/\n  --> {}",
                    shown(&relative_path),
                    shown(root.directory),
                    shown(root.directory),
                    shown(&relative_path),
                ),
                help: Some(format!(
                    "move it to {}/{expected}/{}",
                    shown(root.directory),
                    shown(destination)
                )),
            }));
        }
        facts.push(DeclarationFileFact {
            relative_path,
            kind: DeclarationKind::Macro,
            scope_kind: root.scope_kind,
            ownership_root: root.ownership_root.to_owned(),
            owning_path: root.owning_path.map(str::to_owned),
            declaration_root: root.directory.to_owned(),
        });
    }
    Ok(facts)
}
