//! Names the declaration reads compare against.

/// The free-form job setting that can carry credentials, so person reads show only that it is set.
pub(crate) const VARS_SETTING: &str = "vars";
/// What a withheld setting shows instead of its value.
pub(crate) const WITHHELD_SETTING: &str = "[REDACTED]";
/// Settings that name another target; a value that isn't a target name, such as a state path, is withheld.
pub(crate) const DEFER_SETTING_PREFIX: &str = "defer_";
