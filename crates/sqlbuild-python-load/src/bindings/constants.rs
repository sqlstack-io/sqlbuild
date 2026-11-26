//! The keys of each session `run_sqlload_project` takes.

pub(crate) const ADAPTER: &str = "adapter";
pub(crate) const DATABASE: &str = "database";
pub(crate) const OPENER: &str = "opener";
pub(crate) const DESCRIPTION: &str = "description";
/// The runtime module's name under `_native`.
pub(crate) const RUNTIME_SUBMODULE: &str = "sqlload_runtime";
/// The name SQLLoad's engine imports the runtime module by.
pub(crate) const RUNTIME_MODULE: &str = "sqlbuild._native.sqlload_runtime";
/// What `rename` answers: a loader names its columns on the source it fills.
pub(crate) const RENAME_PROBLEM: &str =
    "a @loader takes no rename function; name its columns on the source it fills";
