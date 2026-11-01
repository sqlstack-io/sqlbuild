//! Compile reuse storage, fingerprint, environment and notice constants.

use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;

use crate::bindings::_helpers::compile_delta::driver::DELTA_VERIFY_ENV_VAR;
use crate::bindings::_helpers::frontier::stage_capture::CAPTURE_DIR_ENV_VAR;

/// The directory under the compiler cache directory holding the native reuse store.
pub(crate) const NATIVE_REUSE_DIRECTORY_NAME: &str = "project-reuse-v3";
/// The reuse directories older releases kept stored compiles in.
pub(crate) const RETIRED_REUSE_DIRECTORY_NAMES: [&str; 2] =
    ["project-reuse-v1", "project-reuse-v2"];
pub(crate) const REUSE_MAX_ENTRY_BYTES: i64 = 512 * 1024 * 1024;
pub(crate) const REUSE_MAX_STORED_ENTRIES: i64 = 8;
pub(crate) const REUSE_DISABLE_ENV_VAR: &str = "SQLBUILD_DISABLE_COMPILE_REUSE";
pub(crate) const REUSE_DISABLE_VALUE: &str = "1";
pub(crate) const REUSE_LOGGER_NAME: &str = "sqlbuild.compile.reuse";
pub(crate) const TRACKED_ENVIRONMENT_PREFIXES: [&str; 2] = ["SQLBUILD_", "SQB_"];
/// Sorted; tracked-prefix variables that never invalidate a stored compile.
pub(crate) const UNTRACKED_ENVIRONMENT_NAMES: [&str; 3] = [
    CAPTURE_DIR_ENV_VAR,
    REUSE_DISABLE_ENV_VAR,
    DELTA_VERIFY_ENV_VAR,
];
pub(crate) const MISSING_ENVIRONMENT_VALUE: &str = "\0missing";
pub(crate) const MISSING_FILE_DIGEST: &str = "\0missing";
pub(crate) const PROJECT_CONFIG_FILENAMES: [&str; 2] =
    [PROJECT_CONFIG_FILENAME, "sqlbuild_project.yml"];
/// The `sqlbuild._native` attribute the reuse check reads its racy window from.
pub(crate) const RACY_WINDOW_ATTRIBUTE: &str = "COMPILE_REUSE_RACY_WINDOW_NS";
/// The `sqlbuild._native` attribute the reuse check reads its excluded root directories from.
pub(crate) const EXCLUDED_ROOT_ATTRIBUTE: &str = "COMPILE_REUSE_EXCLUDED_ROOT_DIRECTORIES";
pub(crate) const RACY_WINDOW_NS: i64 = 2_000_000_000;
pub(crate) const MISSING_PATH_MTIME_NS: i64 = -1;
pub(crate) const PROJECT_ROOT_PATH_MTIME_NS: i64 = -2;
pub(crate) const TARGET_DIRECTORY_NAME: &str = "target";
pub(crate) const COMPILED_DIRECTORY_NAME: &str = "compiled";
pub(crate) const MILLISECONDS_PER_SECOND: f64 = 1000.0;
/// Sorted; top-level project directories the reuse snapshot skips.
pub(crate) const EXCLUDED_ROOT_DIRECTORIES: [&str; 16] = [
    ".cache",
    ".fensu",
    ".hg",
    ".idea",
    ".mypy_cache",
    ".nox",
    ".pytest_cache",
    ".ruff_cache",
    ".sqlbuild",
    ".svn",
    ".tox",
    ".venv",
    ".vscode",
    "logs",
    "target",
    "venv",
];
/// Sorted; directories the reuse snapshot skips at any depth.
pub(crate) const EXCLUDED_DIRECTORIES: [&str; 2] = [".git", "__pycache__"];
pub(crate) const PRESENCE_ONLY_FILE_SUFFIXES: [&str; 2] = [".duckdb", ".duckdb.wal"];
pub(crate) const REUSE_HIT_TIMING: &str = "project_reuse_hits";
pub(crate) const REUSE_MISS_TIMING: &str = "project_reuse_misses";
pub(crate) const REUSE_BYPASS_TIMING: &str = "project_reuse_bypasses";
pub(crate) const REUSE_CHECK_TIMING: &str = "project_reuse_check_ms";
pub(crate) const TOTAL_TIMING: &str = "total_ms";
/// The reuse hit notice, split around its elapsed seconds.
pub(crate) const REUSE_HIT_MESSAGE: [&str; 2] =
    ["Inputs unchanged; reused the previous compile (", " s)"];
pub(crate) const REUSE_STORE_NOTICE_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const REUSE_STORE_NOTICE_PATHS: usize = 100_000;
pub(crate) const BYTES_PER_MEBIBYTE: f64 = 1024.0 * 1024.0;
pub(crate) const REUSE_STORE_DONE_PREFIX: &str = "Recorded compile for reuse (";
pub(crate) const REUSE_STORE_SKIPPED_PREFIX: &str =
    "Did not record compile for reuse: files changed while it ran (";
pub(crate) const COMPUTE_LOG_DIRECTORY_NAME: &str = "logs";
pub(crate) const COMPILE_COMMAND_NAME: &str = "compile";
/// The required and optional keyword fields of `CompileReuseAttempt`.
pub(crate) const REUSE_ATTEMPT_FIELDS: (&[&str], &[&str]) = (
    &["outcome", "project_dir", "started"],
    &[
        "check_ms",
        "exit_code",
        "entry_path",
        "native",
        "selected_target",
        "json_output",
    ],
);
/// The variable that disables the compile cache when it holds `COMPILE_CACHE_DISABLE_VALUE`.
pub(crate) const COMPILE_CACHE_DISABLE_ENV_VAR: &str = "SQLBUILD_DISABLE_COMPILE_CACHE";
pub(crate) const COMPILE_CACHE_DISABLE_VALUE: &str = "1";
/// The fact and reference cache directories older releases kept under each retired cache root.
pub(crate) const RETIRED_CACHE_DIRECTORY_NAMES: [&str; 2] = ["facts-v1", "references-v3"];
/// `settings.strict` when only reporting what it would fail on.
pub(crate) const STRICT_REPORT: &str = "report";
/// The strict mode that fails a compile on any partial check.
pub(crate) const STRICT_ON: &str = "on";
/// The error a model whose checks were partial gets under enforced strict.
pub(crate) const STRICT_CODE: &str = "SQBSTRICT001";
/// What clears a strict error.
pub(crate) const STRICT_HELP: &str = "declare, override or catalog each unproven input (a \
    function signature, an input contract or an output type); to list them without failing, set \
    this in sqlbuild_project.toml:\n    [settings]\n    strict = \"report\"";
