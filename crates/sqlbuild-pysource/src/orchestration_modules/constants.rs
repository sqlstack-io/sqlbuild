//! The orchestration authoring calls a module is read for.

/// The decorator or call that declares a calendar.
pub(crate) const CALENDAR_CALL: &str = "calendar";
/// The decorator or call that declares a sensor.
pub(crate) const SENSOR_CALL: &str = "sensor";
/// The sensor condition that names the job whose runs it watches.
pub(crate) const RUN_EVENT_CALL: &str = "run_event";
/// The keyword naming a declaration.
pub(crate) const NAME_KEYWORD: &str = "name";
/// The sensor keyword naming the job it starts.
pub(crate) const RUN_KEYWORD: &str = "run";
/// The `run_event` keyword naming the job it watches.
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
