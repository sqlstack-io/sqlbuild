//! Read one exact run artifact under the project execution directory.

use crate::cost_store::_helpers::artifacts::{ARTIFACT_NAME, directory, load};
use crate::cost_store::errors::CostStoreError;
use sqlbuild_core::cost_report::errors::CostArtifactError;
use sqlbuild_core::cost_report::models::CostRun;
use std::path::Path;

/// Return `None` for a missing artifact; report invalid artifacts and I/O failures distinctly.
pub fn read_cost_run(project_dir: &Path, run_id: &str) -> Result<Option<CostRun>, CostStoreError> {
    let path = directory(project_dir, run_id)?.join(ARTIFACT_NAME);
    if !path.is_file() {
        return Ok(None);
    }
    let run = load(&path)?;
    if run.run_id != run_id {
        return Err(CostArtifactError::new(
            "cost run artifact run ID does not match its directory",
        )
        .into());
    }
    Ok(Some(run))
}
