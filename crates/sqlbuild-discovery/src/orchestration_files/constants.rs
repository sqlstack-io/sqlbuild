//! The keys job and job group files accept, and the bounds on their run controls.

/// Top-level keys of a job file.
pub const JOB_KEYS: [&str; 12] = [
    "name",
    "description",
    "select",
    "exclude",
    "targets",
    "owner",
    "overlap",
    "concurrency_groups",
    "schedule",
    "on_failure",
    "resources",
    "settings",
];
/// Top-level keys of a job group file.
pub const GROUP_KEYS: [&str; 9] = [
    "name",
    "description",
    "jobs",
    "targets",
    "owner",
    "overlap",
    "concurrency_groups",
    "schedule",
    "on_failure",
];
/// The fewest and most reruns `on_failure.retries` allows.
pub const RETRIES_RANGE: (u8, u8) = (1, 10);
/// The shortest and longest `on_failure.retry_delay_seconds`, a day.
pub const RETRY_DELAY_RANGE: (u32, u32) = (0, 86_400);
/// The job setting that moved under `[settings]`.
pub(crate) const CONCURRENCY_KEY: &str = "concurrency";
/// How close an unknown key must be to a known one to be suggested.
pub(crate) const SUGGESTION_CUTOFF: f64 = 0.6;
/// The folders below `orchestration/`, one per declaration kind.
pub const JOBS_DIRECTORY: &str = "jobs";
pub const GROUPS_DIRECTORY: &str = "job_groups";
pub const CALENDARS_DIRECTORY: &str = "calendars";
pub const SENSORS_DIRECTORY: &str = "sensors";
/// The file suffix of job and job group declarations.
pub const TOML_FILE_SUFFIX: &str = ".toml";
/// Each kind as name conflicts word it, and the suffix its names end with.
pub const JOB_KIND: (&str, &str) = ("job", "_job");
pub const GROUP_KIND: (&str, &str) = ("job group", "_job_group");
pub const CALENDAR_KIND: (&str, &str) = ("calendar", "_calendar");
pub const SENSOR_KIND: (&str, &str) = ("sensor", "_sensor");
/// Where jobs, job groups and schedules are declared now, for projects that still use Python.
pub const TOML_JOBS_HELP: &str = "jobs, job groups and schedules are TOML files in orchestration/jobs/ and orchestration/job_groups/; see https://sqlbuild.com/docs/concepts/orchestration/";
/// The folder Python writes bytecode to, which declarations never live in.
pub(crate) const BYTECODE_DIRECTORY: &str = "__pycache__";
/// The prefix of hidden files and folders, and of private support modules.
pub(crate) const HIDDEN_PREFIX: char = '.';
pub(crate) const PRIVATE_PREFIX: char = '_';
