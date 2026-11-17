//! Whether the environment turns `name` on.

use crate::compile_delta::_helpers::driver::ENABLED_VALUE;

/// Whether the environment turns `name` on.
pub fn env_enabled(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| value == ENABLED_VALUE)
}
