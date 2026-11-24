//! Scope layout names.

use crate::scope_index::models::ResourceKind;

/// Python's builder `_ROOTS`, in its insertion order.
pub const RESOURCE_ROOTS: [(ResourceKind, &str); 5] = [
    (ResourceKind::Model, "sql/models"),
    (ResourceKind::Test, "tests/unit"),
    (ResourceKind::Scenario, "tests/scenarios"),
    (ResourceKind::Function, "sql/functions/sql"),
    (ResourceKind::Source, "sql/sources"),
];
/// The global root of seed resources.
pub const SEED_ROOT: &str = "sql/seeds";
/// The global root of Python function resources.
pub const PYTHON_FUNCTION_ROOT: &str = "sql/functions/python";
/// Declaration directories whose ownership root is global.
pub const GLOBAL_DECLARATION_DIRECTORIES: [&str; 3] = ["sql/macros", "sql/enums", "sql/constants"];
/// The project folder holding every project-wide declaration role.
pub const GLOBAL_ROLE_PARENT: &str = "sql";
/// Python's `PATH_SEPARATOR`.
pub const PATH_SEPARATOR: char = '/';
/// Python's `CURRENT_PATH_COMPONENT`.
pub const CURRENT_PATH_COMPONENT: &str = ".";
/// Python's `PARENT_PATH_COMPONENT`.
pub const PARENT_PATH_COMPONENT: &str = "..";
/// The file name a tested macro's folder is resolved through.
pub const MACRO_TEST_LEXICAL_FILE: &str = "__macro_test__.sql";
/// The prefix of the synthetic model identity a lexical path resolves as.
pub const PATH_RESOURCE_PREFIX: &str = "<path:";
/// The suffix of the synthetic model identity a lexical path resolves as.
pub const PATH_RESOURCE_SUFFIX: &str = ">";
/// Python's `QUALIFIED_IDENTITY_SEPARATOR`.
pub const IDENTITY_SEPARATOR: char = ':';
/// The separator between a private declaration's owner and its name.
pub const OWNER_SEPARATOR: char = '.';
