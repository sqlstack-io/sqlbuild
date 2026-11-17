//! Destination-neutral lifecycle-event export defaults and policy vocabulary.

pub const DEFAULT_EVENT_EXPORT_QUEUE_CAPACITY: i64 = 1024;
pub const DEFAULT_EVENT_EXPORT_SHUTDOWN_TIMEOUT_SECONDS: f64 = 2.0;
pub const DEFAULT_EVENT_EXPORT_INVOCATION_TIMEOUT_SECONDS: f64 = 1.0;
pub const DEFAULT_EVENT_EXPORT_NOTIFICATION_QUEUE_CAPACITY: i64 = 256;
pub const DEFAULT_EVENT_EXPORT_HEALTH_INTERVAL_SECONDS: f64 = 30.0;
/// Every lifecycle-event kind a sink can filter on, sorted.
pub const EVENT_EXPORT_KINDS: [&str; 8] = [
    "audit",
    "invocation",
    "microbatch",
    "operation",
    "resource",
    "retry",
    "run",
    "statement",
];
/// The event that starts an invocation, delivered before that invocation's other queued events.
pub const INVOCATION_START_EVENT_TYPE: &str = "invocation_started";
/// Invocation terminals, delivered only after every other queued event.
pub const INVOCATION_TERMINAL_EVENT_TYPES: [&str; 2] =
    ["invocation_completed", "invocation_failed"];
/// Run and invocation outcomes: always admitted, and the only events delivered in the shutdown reserve.
pub const OUTCOME_EVENT_TYPES: [&str; 6] = [
    "invocation_started",
    "invocation_completed",
    "invocation_failed",
    "run_started",
    "run_completed",
    "run_failed",
];
/// The final share of the shutdown budget kept for outcome events once other backlog is dropped.
pub const OUTCOME_RESERVE_FRACTION: f64 = 0.25;
pub const RETRY_EVENT_TYPE: &str = "retry_scheduled";
pub const RETRY_EVENT_KIND: &str = "retry";
pub const FAILED_EVENT_SUFFIX: &str = "_failed";
pub const FAILED_EVENT_PRIORITY: u8 = 3;
pub const TERMINAL_EVENT_PRIORITY: u8 = 2;
pub const RETRY_EVENT_PRIORITY: u8 = 1;
pub const PROGRESS_EVENT_PRIORITY: u8 = 0;
/// Longest exception type name a failure diagnostic carries.
pub const MAX_EXCEPTION_TYPE_LENGTH: usize = 100;
pub const FALLBACK_EXCEPTION_TYPE: &str = "Exception";
/// The failure type reported when an invocation outlives its deadline.
pub const TIMEOUT_ERROR_TYPE: &str = "TimeoutError";
