//! The command planner the server is built with; the CLI owns the grammar and the selector resolver.

use std::collections::{BTreeMap, BTreeSet};

use crate::runs::models::{CommandPlan, PlannerResource};

/// Each published job and job group's exact `kind:name` selection, by name.
pub type PublishedJobs = BTreeMap<String, BTreeSet<String>>;

/// Parse `command` with the CLI's grammar and resolve its selectors over a publication; pure and side-effect free.
pub trait CommandPlanner: Send + Sync {
    /// The verb, exact node keys and validated settings, or why the command is refused; `job:` selects published jobs.
    fn plan(
        &self,
        command: &[String],
        publication: (&[PlannerResource], &PublishedJobs),
    ) -> Result<CommandPlan, String>;
}

/// What the API hands runs to workers with: their Docker images and the command planner.
pub type RunHandoff = (
    Vec<crate::api::models::DockerImageSource>,
    std::sync::Arc<dyn CommandPlanner>,
);
