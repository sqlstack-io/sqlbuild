//! The node counts a finished run records.

use serde::Serialize;

/// How many of a run's nodes ended each way; `not_started` counts planned nodes that never ran.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct RunNodeCounts {
    pub succeeded: i64,
    pub warned: i64,
    pub failed: i64,
    pub skipped: i64,
    pub not_started: i64,
}

/// What the server knows of a run it started; a worker's handle supplies it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerRun {
    pub orchestration_run_id: String,
    pub sqlbuild_user: String,
    pub attempt: i64,
    pub retry_of_run_id: Option<String>,
}

/// How a run ended: it completed, was interrupted or cancelled, or stopped on an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunEnding {
    Completed,
    Interrupted,
    Raised,
}
