//! How long a stop may take, and the exit status of each reason.

use std::time::Duration;

/// Roles get this long to stop on their own; then their processes are killed.
pub(crate) const STOP_GRACE: Duration = Duration::from_millis(800);
/// After the grace period, this long more before the process exits whatever is still running.
pub(crate) const STOP_DEADLINE: Duration = Duration::from_millis(2_500);
/// `q`: a stop that was asked for.
pub(crate) const QUIT_STATUS: i32 = 0;
/// Ctrl+C or SIGINT, as shells report it.
pub(crate) const INTERRUPT_STATUS: i32 = 130;
/// SIGHUP, as shells report it.
pub(crate) const HANGUP_STATUS: i32 = 129;
/// With a run active, its cancellation gets this long before the stop forces it.
pub(crate) const CANCEL_DEADLINE: Duration = Duration::from_secs(30);
/// How often a stop checks whether the runs it cancels have ended.
pub(crate) const CANCEL_POLL: Duration = Duration::from_millis(250);
/// The newest runs a stop looks through for ones to cancel.
pub(crate) const RUN_SCAN: u32 = 100;
/// Who a stop's cancellations name in the audit log.
pub(crate) const DEV_ACTOR: &str = "local-development";
/// The role a stop's cancellations are granted by: the local control store's owner.
pub(crate) const DEV_GRANTING_ROLE: &str = "control-store-owner";
/// Why a stop cancels a run.
pub(crate) const CANCEL_REASON: &str = "sqb dev stopped";
/// SIGTERM, as shells report it.
pub(crate) const TERMINATE_STATUS: i32 = 143;
/// What a forced stop prints on the terminal before it exits.
pub(crate) const FORCED_LINE: &str = "\u{b7} sqb dev stopped (forced)\n";
/// `sqb dev` could not start.
pub(crate) const START_FAILURE_STATUS: i32 = 2;
/// A service failed after `sqb dev` was ready.
pub(crate) const RUN_FAILURE_STATUS: i32 = 1;
/// How long aborted roles get to unwind after the grace period.
pub(crate) const ABORT_WAIT: Duration = Duration::from_millis(300);
/// How long tasks left over after `sqb dev` ends get before the process exits.
pub(crate) const RUNTIME_SHUTDOWN: Duration = Duration::from_millis(200);
