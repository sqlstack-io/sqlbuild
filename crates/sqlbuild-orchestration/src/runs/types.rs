//! The command planner the server is built with; the CLI owns the grammar and the selector resolver.

use crate::credentials::models::PermissionScope;
use crate::runs::models::{
    CommandPlan, PlannerResource, PublicationCache, PublishedJobs, SelectionProblem,
    SelectionVocabulary,
};

/// Index a publication once with the CLI's selector graph, then parse and resolve commands against it.
pub trait CommandPlanner: Send + Sync {
    /// One publication's nodes and jobs, indexed for repeated planning; pure and side-effect free.
    fn prepare(
        &self,
        resources: Vec<PlannerResource>,
        jobs: PublishedJobs,
    ) -> std::sync::Arc<dyn PreparedPublication>;

    /// The publications this planner has prepared, kept between requests because snapshots never change.
    fn publications(&self) -> &PublicationCache;
}

/// One indexed publication: parse `command` with the CLI's grammar and resolve its selectors over it.
pub trait PreparedPublication: std::fmt::Debug + Send + Sync {
    /// The verb, exact node keys and validated settings, or why the command is refused; `job:` selects published jobs.
    fn plan(&self, command: &[String]) -> Result<CommandPlan, String>;

    /// The sorted keys `sqb build --select <select> --exclude <exclude>` selects, or why it refuses them; each text holds space-separated selectors.
    fn evaluate(&self, select: &str, exclude: &str) -> Result<Vec<String>, Box<SelectionProblem>>;

    /// Every value each selector type takes here, with its node count; `snapshot_id` is left for the caller.
    fn vocabulary(&self) -> SelectionVocabulary;
}

/// Which runs of one location and target to list.
#[derive(Clone, Copy, Debug)]
pub enum RunsListing {
    /// Newest first: at most this many, after the queue time and ID of a previous page's last run.
    Newest(u32, Option<(chrono::DateTime<chrono::Utc>, uuid::Uuid)>),
    /// Queued runs in claim order, at most this many, each with its queue state.
    Queue(u32),
}

/// What the API hands runs to workers with: their Docker images and the command planner.
pub type RunHandoff = (
    Vec<crate::api::models::DockerImageSource>,
    std::sync::Arc<dyn CommandPlanner>,
);

/// A permission a launch needs, and the pinned job whose settings it is needed for, if any.
pub(crate) type ExecutionScope = (PermissionScope, Option<String>);
