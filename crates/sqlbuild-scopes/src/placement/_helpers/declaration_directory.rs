//! The project-relative folder where a declaration with a given placement lives.

use crate::placement::constants::{AUTHORED_ROOT_PATHS, DECLARATION_GROUP_DIRECTORY};
use crate::scope_index::constants::GLOBAL_ROLE_PARENT;
use crate::scope_index::models::{DeclarationKind, ScopeKind};

/// Python's `declaration_directory`.
#[must_use]
pub(crate) fn declaration_directory(
    kind: DeclarationKind,
    scope: ScopeKind,
    path: Option<&str>,
) -> String {
    let parts: &[&str] = kind.role_parts();
    let Some(path) = path.filter(|_| scope != ScopeKind::Global) else {
        return format!("{GLOBAL_ROLE_PARENT}/{}", parts.join("/"));
    };
    let prefix: &str = if scope == ScopeKind::Local { "_" } else { "" };
    let mut role: String = format!("{prefix}{}", parts[0]);
    for part in &parts[1..] {
        role.push('/');
        role.push_str(part);
    }
    if AUTHORED_ROOT_PATHS.contains(&path) {
        return format!("{path}/{role}");
    }
    format!("{path}/{DECLARATION_GROUP_DIRECTORY}/{role}")
}

/// A macro file's kind folder, `python/` or `sql/` by its suffix; nothing for other kinds.
#[must_use]
pub(crate) fn declaration_file_directory(
    kind: DeclarationKind,
    scope: ScopeKind,
    path: Option<&str>,
    file_path: &str,
) -> String {
    let directory: String = declaration_directory(kind, scope, path);
    if kind != DeclarationKind::Macro {
        return directory;
    }
    let folder: &str = if file_path.ends_with(".py") {
        "python"
    } else {
        "sql"
    };
    format!("{directory}/{folder}")
}
