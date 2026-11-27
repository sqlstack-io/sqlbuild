//! The command planner the server is built with; the CLI owns the grammar and the selector resolver.

use crate::credentials::models::PermissionScope;
use std::collections::{BTreeMap, BTreeSet};

use crate::runs::models::{
    CommandPlan, PastRunResults, PlannerResource, PublicationCache, PublishedJobs,
    SelectionProblem, SelectionVocabulary,
};

/// Each past run a command's selectors name, by the run ID as typed.
pub type PastRuns = BTreeMap<String, PastRunResults>;

/// Index one publication's nodes and jobs for repeated planning; pure and side-effect free, so it runs on a blocking thread.
pub type Preparer =
    fn(Vec<PlannerResource>, PublishedJobs) -> std::sync::Arc<dyn PreparedPublication>;

/// Index a publication once with the CLI's selector graph, then parse and resolve commands against it.
pub trait CommandPlanner: Send + Sync {
    /// The function that indexes a publication, which the server calls off its async workers.
    fn preparer(&self) -> Preparer;

    /// One publication's nodes and jobs, indexed for repeated planning.
    fn prepare(
        &self,
        resources: Vec<PlannerResource>,
        jobs: PublishedJobs,
    ) -> std::sync::Arc<dyn PreparedPublication> {
        (self.preparer())(resources, jobs)
    }

    /// The publications this planner has prepared, kept between requests because snapshots never change.
    fn publications(&self) -> &PublicationCache;
}

/// One indexed publication: parse `command` with the CLI's grammar and resolve its selectors over it.
pub trait PreparedPublication: std::fmt::Debug + Send + Sync {
    /// The run IDs, as typed, that `run:`, `failed:` and `skipped:` selectors in `texts` name, so the caller can read those runs.
    fn run_selector_ids(&self, texts: &[String]) -> BTreeSet<String>;

    /// The verb, exact node keys and validated settings, or why the command is refused; `job:` selects published jobs and run selectors the `past` runs.
    fn plan(&self, command: &[String], past: &PastRuns) -> Result<CommandPlan, String>;

    /// The sorted keys `sqb build --select <select> --exclude <exclude>` selects, given `(select, exclude)`, or why it refuses them; each text holds space-separated selectors.
    fn evaluate(
        &self,
        selectors: (&str, &str),
        past: &PastRuns,
    ) -> Result<Vec<String>, Box<SelectionProblem>>;

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
