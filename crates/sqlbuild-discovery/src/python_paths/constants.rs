//! The documented roots project Python may live under, and the directories the walk skips.

use crate::constants::ORCHESTRATION_DIRECTORY;

/// Project folders whose Python files are always supported, as path parts.
pub(crate) const PYTHON_ROOTS: [&[&str]; 8] = [
    &["python"],
    &["rules"],
    &[ORCHESTRATION_DIRECTORY],
    &["sql", "macros"],
    &["sql", "enums"],
    &["sql", "constants"],
    &["sql", "functions", "python"],
    &["sql", "hooks", "python"],
];
/// Top-level directories the walk does not enter.
pub(crate) const IGNORED_ROOTS: [&str; 4] = ["logs", "target", "tests", "venv"];
/// Directories the walk never enters.
pub(crate) const PYTHON_CACHE_DIRECTORY: &str = "__pycache__";
/// Grouped hook roles whose Python files are supported inside an authored tree.
pub(crate) const GROUPED_PYTHON_HOOK_ROLES: [[&str; 3]; 2] = [
    ["_sqlbuild", "hooks", "python"],
    ["_sqlbuild", "_hooks", "python"],
];
/// The prefix of a hidden directory the walk skips.
pub(crate) const HIDDEN_PREFIX: &str = ".";
/// The folders holding project Python tests, each mirroring the project folder its second part names.
pub(crate) const PYTHON_TEST_ROOTS: [&[&str]; 3] = [
    &["tests", ORCHESTRATION_DIRECTORY],
    &["tests", "python", "providers"],
    &["tests", "python", "nodes"],
];
