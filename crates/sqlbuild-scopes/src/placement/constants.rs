//! Folder names and labels the placement rules read and print.

/// The project root as a placement anchor path.
pub(crate) const PROJECT_ROOT_PATH: &str = ".";
/// The folder grouping declaration roles below a non-root owner.
pub(crate) const DECLARATION_GROUP_DIRECTORY: &str = "_sqlbuild";
/// Owners whose declaration roles sit directly below them.
pub(crate) const AUTHORED_ROOT_PATHS: &[&str] = &[
    "sql/models",
    "tests/unit",
    "tests/scenarios",
    "sql/functions/sql",
    "sql/sources",
];
