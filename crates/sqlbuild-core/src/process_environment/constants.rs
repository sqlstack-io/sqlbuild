//! The variables that tell child processes which env file their parent loaded, or why it could not.

/// Set on every child: the project env file this process tree loaded and the names it took from it.
pub const ENV_FILE_MARKER_VARIABLE: &str = "SQLBUILD_ENV_FILE_LOADED";

/// Set on a run worker whose env file can't be read: the reason, naming the line but never a value.
pub const ENV_FILE_ERROR_VARIABLE: &str = "SQLBUILD_ENV_FILE_ERROR";

/// Set on a run worker: the bearer the run reports with.
pub const RUN_TOKEN_VARIABLE: &str = "SQLBUILD_RUN_TOKEN";
