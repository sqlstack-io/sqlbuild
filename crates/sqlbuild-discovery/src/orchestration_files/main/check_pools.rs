//! Refuse a job or job group taking a pool the project does not declare, or more of it than its limit.

use sqlbuild_config::project::models::PoolLimits;

use crate::orchestration_files::_helpers::inventory_files::failure;
use crate::orchestration_files::_helpers::pools::pool_problem;
use crate::orchestration_files::errors::FileError;
use crate::orchestration_files::models::{OrchestrationFailure, OrchestrationInventory};

/// The first job or job group taking an undeclared pool, or more slots than a target it runs in allows.
pub fn check_pools(
    inventory: &OrchestrationInventory,
    limits: &PoolLimits,
) -> Result<(), OrchestrationFailure> {
    let runs = inventory
        .jobs
        .iter()
        .map(|(path, job)| (path, ("job", &job.run)))
        .chain(
            inventory
                .groups
                .iter()
                .map(|(path, group)| (path, ("job group", &group.run))),
        );
    for (path, (kind, run)) in runs {
        if let Some((message, help)) = pool_problem((kind, run), limits) {
            return Err(failure(
                path,
                FileError {
                    message,
                    position: run.name.position,
                    help: Some(help),
                },
            ));
        }
    }
    Ok(())
}
