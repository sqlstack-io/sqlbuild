//! The controls of a TOML job or group; discovery has already checked the owner and the retry bounds.

use crate::orchestration_reads::_helpers::orchestration_declarations;
use crate::orchestration_reads::models::RunControls;
use sqlbuild_discovery::orchestration_files::models::RunFile;

/// The controls of a TOML job or group; discovery has already checked the owner and the retry bounds.
#[inline]
pub fn run_controls(run: &RunFile) -> RunControls {
    orchestration_declarations::run_controls(run)
}
