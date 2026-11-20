//! Replace this process's environment overlay.

use crate::process_environment::_helpers::overlay::replace;
use crate::process_environment::models::EnvironmentOverlay;

/// Make `overlay` what in-process reads and every later child see.
pub fn install_environment_overlay(overlay: EnvironmentOverlay) {
    replace(overlay);
}
