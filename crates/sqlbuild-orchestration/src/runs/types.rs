//! The command planner the server is built with; the CLI owns the grammar and the selector resolver.

use crate::runs::models::{CommandPlan, PlannerResource};

/// Parse `command` with the CLI's grammar and resolve its selectors over `resources`; pure and side-effect free.
pub trait CommandPlanner: Send + Sync {
    /// The verb, exact node keys and validated settings, or why the command is refused.
    fn plan(
        &self,
        command: &[String],
        resources: &[PlannerResource],
    ) -> Result<CommandPlan, String>;
}

/// What the API hands runs to workers with: their Docker images and the command planner.
pub type RunHandoff = (
    Vec<crate::api::models::DockerImageSource>,
    std::sync::Arc<dyn CommandPlanner>,
);
