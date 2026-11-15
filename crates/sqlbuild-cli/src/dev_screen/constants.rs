//! Labels, bounds and keys of the `sqb dev` screen.

use std::time::Duration;

use crate::dev_screen::models::DevStep;

/// Detailed log lines kept in memory for `l`; older lines drop off the front.
pub(crate) const LOG_RING_LINES: usize = 500;
/// The keys the hotkey bar shows, in order.
pub(crate) const HOTKEYS: [(char, &str); 5] = [
    ('o', "open UI"),
    ('r', "recompile"),
    ('s', "retry warehouse"),
    ('l', "logs"),
    ('q', "quit"),
];
/// The note under a failed generation's event line.
pub(crate) const FAILED_HINT: &str = "r retry \u{b7} l full log";
/// The snapshot ID characters a published line shows.
pub(crate) const SNAPSHOT_ID_CHARS: usize = 8;
/// The checklist's steps, in the order they are drawn.
pub(crate) const STEPS: [DevStep; 7] = [
    DevStep::ControlStore,
    DevStep::Api,
    DevStep::Scheduler,
    DevStep::Discover,
    DevStep::Compile,
    DevStep::Publish,
    DevStep::Warehouse,
];
/// The framed sign-in block's label while the adapter is not yet known.
pub(crate) const WAREHOUSE_FRAME_LABEL: &str = "warehouse";
/// How often the hotkey thread checks whether it may read stdin.
pub(crate) const KEY_POLL: Duration = Duration::from_millis(50);
/// How often the running step's spinner and timer redraw.
pub(crate) const TICK: Duration = Duration::from_millis(100);
/// Log lines printed when `sqb dev` stops on an error, so the cause stays on screen.
pub(crate) const FAILURE_LOG_LINES: usize = 40;
/// The statuses of SQLBuild's own progress rows, which the framed block leaves out.
pub(crate) const LIFECYCLE_STATUSES: [&str; 3] = ["START", "OK", "FAIL"];
/// The `sqb debug` row that says whether the warehouse answered.
pub(crate) const CONNECTION_TEST_LABEL: &str = "connection test";
/// The warehouse state when `sqb debug` printed no report.
pub(crate) const NO_REPORT: &str = "sqb debug did not report a connection; see l for its log";
