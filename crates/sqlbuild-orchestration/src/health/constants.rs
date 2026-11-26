//! Health pacing, and the names and sentences the health report uses.

use std::time::Duration;

/// How often each server checks its control database, and how often clients should ask again while it is down.
pub const HEALTH_PROBE_INTERVAL: Duration = Duration::from_secs(10);
/// The failed requests a component keeps for administrators.
pub(crate) const KEPT_REQUEST_IDS: usize = 5;

pub(crate) const CONTROL_DATABASE_NAME: &str = "Control database";
pub(crate) const SCHEDULER_NAME: &str = "Scheduler";
pub(crate) const SERVER_NAME: &str = "Server";
/// The control database's reason before this server has reached it.
pub(crate) const CONNECTING: &str = "Connecting to the control database";
/// The scheduler's reason before its first cycle.
pub(crate) const SCHEDULER_STARTING: &str = "Starting";
/// What any part that reads the control database says while it can't.
pub(crate) const WAITING_FOR_DATABASE: &str = "Waiting for the control database";

/// What a host is called when its URL names none, such as a local socket.
pub(crate) const UNNAMED_HOST: &str = "the configured host";
/// What a login is called when its URL names no user.
pub(crate) const UNNAMED_USER: &str = "the configured user";
/// Fragments of the resolver's own messages for a name that doesn't resolve, on each platform.
pub(crate) const NAME_LOOKUP_MESSAGES: [&str; 5] = [
    "failed to lookup address",
    "name or service not known",
    "temporary failure in name resolution",
    "nodename nor servname",
    "no such host is known",
];
/// The SQLSTATEs of a refused login.
pub(crate) const LOGIN_REJECTED_CODES: [&str; 2] = ["28P01", "28000"];
/// The SQLSTATE of a database with no free connections.
pub(crate) const TOO_MANY_CONNECTIONS_CODE: &str = "53300";
/// The SQLSTATEs of a database shutting down or not yet accepting connections.
pub(crate) const RESTARTING_CODES: [&str; 3] = ["57P01", "57P02", "57P03"];
/// The SQLSTATE class of a lost or failed connection.
pub(crate) const CONNECTION_CLASS: &str = "08";
