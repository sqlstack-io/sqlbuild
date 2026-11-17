//! Public constants of frontier.

pub const CACHE_DIRECTORY_NAME: &str = "cache";

pub const COMPILER_CACHE_DIRECTORY_NAME: &str = "compiler";

/// The namespace suffix of the current native compiler store.
pub const NATIVE_CACHE_NAMESPACE_SUFFIX: &str = "-native-v1";

/// The namespace suffixes of compiler store roots older releases wrote.
pub const RETIRED_CACHE_NAMESPACE_SUFFIXES: [&str; 3] = ["", "-native-v1", "-native-preview-v1"];

pub const CAPTURE_DIR_ENV_VAR: &str = "SQLBUILD_COMPILER_STAGE_CAPTURE_DIR";
