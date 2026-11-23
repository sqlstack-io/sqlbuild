//! Whether only the first DSL header of a file is lintable.

use crate::lint_text::constants::{
    DECLARATION_GROUP_DIRECTORY, MODELS_DIRECTORY, SCOPED_DECLARATION_DIRECTORIES,
};

/// Model files outside declaration folders hold one header; every other file may hold several.
#[must_use]
pub fn first_header_only(relative_parts: &[String]) -> bool {
    if relative_parts.len() <= MODELS_DIRECTORY.len()
        || relative_parts
            .iter()
            .zip(MODELS_DIRECTORY)
            .any(|(part, root)| part != root)
    {
        return false;
    }
    let rest: &[String] = &relative_parts[MODELS_DIRECTORY.len()..];
    let folders: &[String] = rest.split_last().map_or(&[], |(_, folders)| folders);
    !folders.iter().any(|folder| {
        folder == DECLARATION_GROUP_DIRECTORY
            || SCOPED_DECLARATION_DIRECTORIES.contains(&folder.as_str())
    })
}
