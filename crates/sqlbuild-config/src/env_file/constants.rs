//! The env file a project loads when it sets none.

/// `env_file` when neither the project nor the selected target sets one.
pub(crate) const DEFAULT_ENV_FILE: &str = ".env";
/// Permission bits that let the group or others read a file.
#[cfg(unix)]
pub(crate) const SHARED_READ_BITS: u32 = 0o044;
