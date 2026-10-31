//! The documented roots project Python may live under, and the directories the walk skips.

use crate::constants::ORCHESTRATION_DIRECTORY;

/// Top-level directories whose Python files are always supported.
pub(crate) const DIRECT_PYTHON_ROOTS: [&str; 11] = [
    "adapters",
    "materializations",
    "providers",
    "python",
    "rules",
    "sinks",
    "macros",
    "enums",
    "constants",
    "event_exporters",
    ORCHESTRATION_DIRECTORY,
];
/// `(root, language)` directory pairs whose Python files are supported.
pub(crate) const LANGUAGE_PYTHON_ROOTS: [(&str, &str); 2] =
    [("functions", "python"), ("hooks", "python")];
/// Top-level directories the walk does not enter.
pub(crate) const IGNORED_ROOTS: [&str; 4] = ["logs", "target", "tests", "venv"];
/// Root-level Python files that are supported.
pub(crate) const SUPPORTED_ROOT_FILES: [&str; 1] = ["adapter.py"];
/// Directories the walk never enters.
pub(crate) const PYTHON_CACHE_DIRECTORY: &str = "__pycache__";
/// Grouped hook roles whose Python files are supported inside an authored tree.
pub(crate) const GROUPED_PYTHON_HOOK_ROLES: [[&str; 3]; 2] = [
    ["_sqlbuild", "hooks", "python"],
    ["_sqlbuild", "_hooks", "python"],
];
/// The prefix of a hidden directory the walk skips.
pub(crate) const HIDDEN_PREFIX: &str = ".";
