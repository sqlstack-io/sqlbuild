//! Validate scoped `_sqlbuild` declaration groups and list the groups below each owner.

use crate::constants::{
    CANONICAL_AUTHORED_ROOTS, DECLARATION_GROUP_DIRECTORY, GROUPED_NAMED_DECLARATION_DIRECTORIES,
    SCOPED_DECLARATION_DIRECTORIES,
};
use crate::declarations::_helpers::paths::parent;
use crate::declarations::_helpers::scan::{declaration_failure, shown, shown_list};
use crate::declarations::errors::ScanError;
use crate::declarations::models::DeclarationGroup;
use crate::models::StageFailure;
use crate::tree::main::children::children;
use crate::tree::main::rglob::rglob;
use crate::tree::models::{ProjectTree, TreeEntry};

/// Every `_sqlbuild/` directory below a canonical root, root by root, in path order.
fn all_groups(tree: &ProjectTree) -> Result<Vec<DeclarationGroup>, StageFailure> {
    let mut groups: Vec<DeclarationGroup> = Vec::new();
    for root_parts in CANONICAL_AUTHORED_ROOTS {
        let root: String = root_parts.join("/");
        if !tree.is_dir(&root) {
            continue;
        }
        for directory in rglob(tree, &root, |entry| {
            entry.name == DECLARATION_GROUP_DIRECTORY
        })? {
            if tree.is_dir(&directory) {
                groups.push(DeclarationGroup {
                    root: root.clone(),
                    directory,
                });
            }
        }
    }
    Ok(groups)
}

/// The declaration groups below a concrete owner directory.
pub(crate) fn owned_groups(tree: &ProjectTree) -> Result<Vec<DeclarationGroup>, StageFailure> {
    Ok(all_groups(tree)?
        .into_iter()
        .filter(|group| parent(&group.directory) != group.root)
        .collect())
}

pub(crate) fn validate_declaration_groups(tree: &ProjectTree) -> Result<(), ScanError> {
    if tree.exists(DECLARATION_GROUP_DIRECTORY) {
        return Err(declaration_failure(format!(
            "Grouped declaration root {DECLARATION_GROUP_DIRECTORY}/ must be below a canonical \
             authored root"
        )));
    }
    for group in all_groups(tree)? {
        if parent(&group.directory) == group.root {
            return Err(declaration_failure(format!(
                "Grouped declaration root {}/ must be below a concrete owner directory; use the \
                 project-wide sql/macros/, sql/enums/, sql/constants/, sql/audits/, sql/schemas/, \
                 or sql/hooks/ root instead",
                shown(&group.directory)
            )));
        }
        let unsupported: Vec<String> = children(tree, &group.directory)?
            .into_iter()
            .filter(|(_, entry)| !is_group_role(entry))
            .map(|(path, _)| path)
            .collect();
        if !unsupported.is_empty() {
            return Err(declaration_failure(format!(
                "Declaration group {}/ contains unsupported entries: {}",
                shown(&group.directory),
                shown_list(&unsupported)
            )));
        }
    }
    Ok(())
}

fn is_group_role(entry: &TreeEntry) -> bool {
    entry.is_dir
        && (SCOPED_DECLARATION_DIRECTORIES.contains(&entry.name.as_str())
            || GROUPED_NAMED_DECLARATION_DIRECTORIES.contains(&entry.name.as_str()))
}
