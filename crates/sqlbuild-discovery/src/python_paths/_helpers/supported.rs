//! Whether one project Python file lives under a documented executable root.

use crate::constants::{CANONICAL_AUTHORED_ROOTS, SCOPED_DECLARATION_DIRECTORIES};
use crate::python_paths::constants::{GROUPED_PYTHON_HOOK_ROLES, PYTHON_ROOTS};

/// Whether the file at relative `parts` is supported project Python.
pub(crate) fn is_supported(parts: &[String]) -> bool {
    if PYTHON_ROOTS
        .iter()
        .any(|root| parts.len() > root.len() && starts_with(parts, root))
    {
        return true;
    }
    scoped_declaration_path(parts)
}

/// Whether the file sits in a scoped declaration or grouped hook directory of an authored root.
fn scoped_declaration_path(parts: &[String]) -> bool {
    for root in CANONICAL_AUTHORED_ROOTS {
        if !starts_with(parts, root) {
            continue;
        }
        let end: usize = parts.len().saturating_sub(1).max(root.len());
        let scoped: &[String] = &parts[root.len()..end];
        for (index, part) in scoped.iter().enumerate() {
            if SCOPED_DECLARATION_DIRECTORIES.contains(&part.as_str()) {
                return true;
            }
            for role in GROUPED_PYTHON_HOOK_ROLES {
                if starts_with(&scoped[index..], &role) {
                    return true;
                }
            }
        }
        return false;
    }
    false
}

/// Whether `parts` begins with every name of `prefix`.
fn starts_with(parts: &[String], prefix: &[&str]) -> bool {
    parts.len() >= prefix.len() && parts.iter().zip(prefix).all(|(part, name)| part == name)
}
