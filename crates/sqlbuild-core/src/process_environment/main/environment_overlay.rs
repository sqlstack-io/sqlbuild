//! This process's environment overlay, for applying to a child's command.

use crate::process_environment::_helpers::overlay::current;
use crate::process_environment::models::EnvironmentOverlay;

/// The variables every child this process starts must have set or unset.
#[must_use]
pub fn environment_overlay() -> EnvironmentOverlay {
    current()
}
