//! Warnings for deprecated `[settings]` keys that still load as aliases.

use crate::project::errors::LoadFailure;
use crate::project::models::ConfigPaths;

/// One `Warning:` line per configuration file that sets a deprecated setting alias.
pub fn deprecated_setting_warnings(paths: &ConfigPaths) -> Result<Vec<String>, LoadFailure> {
    crate::project::_helpers::documents::deprecated_setting_warnings(paths)
}
