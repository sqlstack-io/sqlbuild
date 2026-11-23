//! Validate where named declaration roots may appear in the project layout.

use crate::constants::{
    AUDIT_ROLE_DIRECTORY, AUDIT_ROLE_PATH, DECLARATION_GROUP_DIRECTORY,
    GLOBAL_DECLARATION_DIRECTORIES, GLOBAL_NAMED_DECLARATION_DIRECTORIES, GLOBAL_ROLE_PARENT,
    LOCAL_AUDIT_ROLE_DIRECTORY, LOCAL_DECLARATION_DIRECTORIES, SINGULAR_AUDIT_DIRECTORY,
};
use crate::declarations::_helpers::declaration_groups::owned_groups;
use crate::declarations::_helpers::paths::{join, name};
use crate::declarations::_helpers::scan::{declaration_failure, shown, shown_list};
use crate::declarations::errors::ScanError;
use crate::declarations::models::DeclarationGroup;
use crate::tree::main::children::children;
use crate::tree::main::directories::directories;
use crate::tree::models::ProjectTree;

const AUDIT_ROLES: &[&str] = &["generic", SINGULAR_AUDIT_DIRECTORY];
const LOCAL_AUDIT_ROLES: &[&str] = &["generic"];
const HOOK_ROLES: &[&str] = &["python", "sql"];

/// One role directory and the child directories it accepts, in sorted order.
struct RoleChildren<'a> {
    directory: &'a str,
    allowed: &'a [&'a str],
    help_text: &'a str,
}

/// The declaration groups the named roles use, once every named role layout is valid.
pub(crate) fn named_declaration_groups(
    tree: &ProjectTree,
) -> Result<Vec<DeclarationGroup>, ScanError> {
    if tree.is_dir(AUDIT_ROLE_PATH) {
        require_role_children(
            tree,
            &RoleChildren {
                directory: AUDIT_ROLE_PATH,
                allowed: AUDIT_ROLES,
                help_text: "audit files must live in sql/audits/generic/ or sql/audits/singular/",
            },
        )?;
    }
    let mut top_levels: Vec<&str> = GLOBAL_NAMED_DECLARATION_DIRECTORIES.to_vec();
    top_levels.sort_unstable();
    for name_of_role in top_levels {
        let top_level: String = join(GLOBAL_ROLE_PARENT, name_of_role);
        if !tree.is_dir(&top_level) {
            continue;
        }
        if let Some(nested) = directories(tree, &top_level)?
            .into_iter()
            .find(|directory| is_nested_role_directory(name(directory)))
        {
            return Err(declaration_failure(format!(
                "Declaration directory {}/ is not allowed inside the project-wide \
                 {top_level}/ role; scoped declarations belong under \
                 <folder>/{DECLARATION_GROUP_DIRECTORY}/ below a resource tree",
                shown(&nested)
            )));
        }
    }
    let groups: Vec<DeclarationGroup> = owned_groups(tree)?;
    for group in &groups {
        validate_group_roles(tree, &group.directory)?;
    }
    Ok(groups)
}

fn validate_group_roles(tree: &ProjectTree, group: &str) -> Result<(), ScanError> {
    let roles: [(&str, &[&str]); 4] = [
        (AUDIT_ROLE_DIRECTORY, AUDIT_ROLES),
        (LOCAL_AUDIT_ROLE_DIRECTORY, LOCAL_AUDIT_ROLES),
        ("hooks", HOOK_ROLES),
        ("_hooks", HOOK_ROLES),
    ];
    for (role, allowed) in roles {
        let directory: String = join(group, role);
        if !tree.is_dir(&directory) {
            continue;
        }
        let singular: String = join(&directory, SINGULAR_AUDIT_DIRECTORY);
        if role == LOCAL_AUDIT_ROLE_DIRECTORY && tree.exists(&singular) {
            return Err(declaration_failure(format!(
                "{}/ is invalid: singular audits are never used by name, so folder-only \
                 visibility has no meaning; use {DECLARATION_GROUP_DIRECTORY}/audits/singular/",
                shown(&singular)
            )));
        }
        let help_text: String = format!(
            "{role}/ accepts only {}",
            allowed
                .iter()
                .map(|name| format!("{name}/"))
                .collect::<Vec<String>>()
                .join(", ")
        );
        require_role_children(
            tree,
            &RoleChildren {
                directory: &directory,
                allowed,
                help_text: &help_text,
            },
        )?;
    }
    Ok(())
}

fn require_role_children(tree: &ProjectTree, role: &RoleChildren<'_>) -> Result<(), ScanError> {
    let unsupported: Vec<String> = children(tree, role.directory)?
        .into_iter()
        .filter(|(_, entry)| {
            !entry.name.starts_with('.')
                && (!entry.is_dir || !role.allowed.contains(&entry.name.as_str()))
        })
        .map(|(path, _)| path)
        .collect();
    if unsupported.is_empty() {
        return Ok(());
    }
    Err(declaration_failure(format!(
        "Unsupported entries in {}/: {}; {}",
        shown(role.directory),
        shown_list(&unsupported),
        role.help_text
    )))
}

fn is_nested_role_directory(directory_name: &str) -> bool {
    directory_name == DECLARATION_GROUP_DIRECTORY
        || GLOBAL_DECLARATION_DIRECTORIES.contains(&directory_name)
        || LOCAL_DECLARATION_DIRECTORIES.contains(&directory_name)
}
