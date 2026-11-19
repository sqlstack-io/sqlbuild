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
    "pools",
    "schedule",
    "retry",
    "resources",
    "settings",
];
/// Top-level keys of a job group file.
pub const GROUP_KEYS: [&str; 10] = [
    "name",
    "description",
    "jobs",
    "targets",
    "owner",
    "overlap",
    "pools",
    "schedule",
    "retry",
    "dependencies",
];
/// The group table that sets how members wait for and react to the jobs they depend on.
pub(crate) const DEPENDENCIES_KEY: &str = "dependencies";
/// The `[dependencies]` setting for when a member starts, and its values.
pub(crate) const WAIT_FOR_KEY: &str = "wait_for";
pub(crate) const WAIT_FOR_VALUES: [&str; 2] = ["job", "node"];
/// The `[dependencies]` setting for a failed dependency, and its values.
pub(crate) const DEPENDENCY_FAILURE_KEY: &str = "on_failure";
pub(crate) const DEPENDENCY_FAILURE_VALUES: [&str; 3] = ["partial", "skip", "run"];
/// The fewest and most runs `retry.attempts` allows, the first run included.
pub const ATTEMPTS_RANGE: (u8, u8) = (2, 11);
/// The shortest and longest `retry.delay_seconds`, a day.
pub const RETRY_DELAY_RANGE: (u32, u32) = (0, 86_400);
/// The retry table, and the keys of its run count and delay.
pub(crate) const RETRY_TABLE: &str = "retry";
pub(crate) const RETRY_ATTEMPTS_KEY: &str = "attempts";
pub(crate) const RETRY_DELAY_KEY: &str = "delay_seconds";
/// The removed retry table, and how to write it as `[retry]`.
pub(crate) const REMOVED_RETRY_TABLE: (&str, &str) = (
    "on_failure",
    "`[on_failure]` is now `[retry]`; `attempts = retries + 1`, \
     `delay_seconds = retry_delay_seconds`",
);
/// The job setting that moved under `[settings]`.
pub(crate) const CONCURRENCY_KEY: &str = "concurrency";
/// The most slots a job or job group may take of one pool.
pub(crate) const MAX_POOL_AMOUNT: i64 = 10_000;
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
/// The folders of a module path before the ones below its role folder: `orchestration` and the role.
pub(crate) const ROLE_FOLDER_DEPTH: usize = 2;
/// Every folder `orchestration/` may hold.
pub const ROLE_DIRECTORIES: [&str; 4] = [
    JOBS_DIRECTORY,
    GROUPS_DIRECTORY,
    CALENDARS_DIRECTORY,
    SENSORS_DIRECTORY,
];
