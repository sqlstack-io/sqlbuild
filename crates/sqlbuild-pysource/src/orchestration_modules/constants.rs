//! The orchestration authoring calls a module is read for.

/// The decorator or call that declares a calendar.
pub(crate) const CALENDAR_CALL: &str = "calendar";
/// The decorator or call that declares a sensor.
pub(crate) const SENSOR_CALL: &str = "sensor";
/// The sensor condition that names the job whose runs it watches.
pub(crate) const RUN_EVENT_CALL: &str = "run_event";
/// The sensor condition that watches platform events and selects no nodes.
pub(crate) const PLATFORM_EVENT_CALL: &str = "platform_event";
/// The sensor keyword holding its one condition.
pub(crate) const WHEN_KEYWORD: &str = "when";
/// The longest source excerpt a finding quotes.
pub(crate) const EXCERPT_LIMIT: usize = 60;
/// Sensor conditions that select the nodes or sources they watch.
pub(crate) const SELECTING_CONDITION_CALLS: [&str; 4] = [
    "node_event",
    "check_event",
    "source_new_data",
    "source_stale",
];
/// The keyword a selecting condition takes its selection in.
pub(crate) const SELECT_KEYWORD: &str = "select";
/// The keyword naming a declaration.
pub(crate) const NAME_KEYWORD: &str = "name";
/// The record a sensor's function returns to start a job.
pub(crate) const RUN_REQUEST_CALL: &str = "RunRequest";
/// The keyword naming the job a `run_event` watches or a `RunRequest` starts.
pub(crate) const JOB_KEYWORD: &str = "job";
/// The sensor keyword naming the targets it publishes for.
pub(crate) const TARGETS_KEYWORD: &str = "targets";
/// The public module the authoring API is imported from.
pub(crate) const ORCHESTRATION_MODULE: &str = "sqlbuild.orchestration";
/// Authoring names that TOML declarations replaced.
pub(crate) const REMOVED_NAMES: [&str; 6] = [
    "job",
    "job_group",
    "schedule",
    "calendar_schedule",
    "on_failure",
    "resources",
];
