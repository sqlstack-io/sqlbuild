//! The variable that tells child processes which env file their parent loaded.

/// Set on every child: the project env file this process tree loaded and the names it took from it.
pub const ENV_FILE_MARKER_VARIABLE: &str = "SQLBUILD_ENV_FILE_LOADED";
