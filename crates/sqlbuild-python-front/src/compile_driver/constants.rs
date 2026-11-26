//! The names a loader's SQLLoad project uses where the project leaves them unset.

/// The target a loader loads into when the project names no target.
pub(crate) const LOADER_TARGET: &str = "default";
/// The target's `loader_schema` when the target has no default schema; each run names its own `destination_schema`.
pub(crate) const LOADER_SCHEMA: &str = "main";
