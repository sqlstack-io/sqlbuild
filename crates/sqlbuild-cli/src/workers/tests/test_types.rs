//! Table rows for local worker supervision.

use crate::workers::models::WorkerExit;

/// A local worker whose process group is registered, and what supervision then sees of its exit.
pub(super) struct RecordExitTestCase {
    pub(super) description: &'static str,
    /// The group the worker leads, unique per case, since the stop switch is shared by the process.
    pub(super) group: u32,
    pub(super) seen: Option<WorkerExit>,
    /// Whether a forced stop still signals the group afterwards.
    pub(super) expected_registered: bool,
}
