//! Whether delta compiles, and so project images, are on for this process.

use crate::compile_delta::constants::DELTA_DISABLE_ENV_VAR;
use crate::compile_delta::main::driver::env_enabled::env_enabled;

/// Whether delta compiles, and so project images, are on for this process.
pub fn delta_enabled() -> bool {
    !env_enabled(DELTA_DISABLE_ENV_VAR)
}
