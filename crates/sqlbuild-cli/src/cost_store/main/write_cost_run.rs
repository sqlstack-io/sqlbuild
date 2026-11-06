//! Atomically replace one native cost run artifact.

use crate::cost_store::_helpers::artifacts::{ARTIFACT_NAME, TEMPORARY_NAME, directory};
use crate::cost_store::errors::CostStoreError;
use sqlbuild_core::cost_report::main::cost_artifact_text::cost_artifact_text;
use sqlbuild_core::cost_report::models::CostRun;
use std::fs;
use std::path::{Path, PathBuf};

/// Persist the native record under its confined run ID and return the artifact path.
pub fn write_cost_run(project_dir: &Path, run: &CostRun) -> Result<PathBuf, CostStoreError> {
    let directory = directory(project_dir, &run.run_id)?;
    fs::create_dir_all(&directory)?;
    let destination = directory.join(ARTIFACT_NAME);
    let temporary = directory.join(TEMPORARY_NAME);
    fs::write(&temporary, cost_artifact_text(run))?;
    fs::rename(&temporary, &destination)?;
    Ok(destination)
}
