//! Public constants of event exporting.

pub(crate) const SCOPE_CONTEXT_VARIABLE: &str = "sqlbuild_event_exporter_command_scope";

pub const DISPATCHER_THREAD_NAME: &str = "sqlbuild-event-exporter-dispatcher";

pub const NOTIFIER_THREAD_NAME: &str = "sqlbuild-event-exporter-notifier";

pub const HEALTH_THREAD_NAME: &str = "sqlbuild-event-exporter-health";

pub const OUTPUT_SINK_THREAD_NAME: &str = "sqlbuild-command-output-sink";

/// Prefix of each exporter's long-lived delivery thread name.
pub const INVOCATION_THREAD_PREFIX: &str = "sqlbuild-event-exporter-";

/// Attribute naming the worker of a long-lived dispatcher thread, for the exit hook.
pub const WORKER_ATTRIBUTE: &str = "_sqlbuild_event_exporter_worker";

/// The longest single wait of a polling delivery loop, as the Python dispatcher polled.
pub const POLL_SECONDS: f64 = 0.01;

/// The most one dispatch round charges a sink call, so the dispatcher's GIL waits are not counted.
pub const CHARGE_LIMIT_SECONDS: f64 = 2.0 * POLL_SECONDS;

/// Extra time `shutdown` waits for the dispatcher thread beyond the drain deadline.
pub const JOIN_GRACE_SECONDS: f64 = 0.05;

/// How long shutdown and interpreter exit wait for each stopped dispatcher thread.
pub const EXIT_JOIN_SECONDS: f64 = 1.0;

/// The most attempts one exporter's delivery thread holds, at most the queue capacity; more stay queued.
pub const LANE_WINDOW: usize = 64;

/// The longest a finalizer waits for sink calls still running after shutdown before it runs anyway.
pub const IDLE_WAIT_SECONDS: f64 = 1.0;

/// No time left before a deadline.
pub const NO_SECONDS: f64 = 0.0;
