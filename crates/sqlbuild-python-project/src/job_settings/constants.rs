//! Constants of named job settings.

/// Ends the options; job flags go before it so they are never read as selectors.
pub const OPTIONS_TERMINATOR: &str = "--";
/// The commands whose selections take the settings of the jobs they name.
pub const JOB_SETTING_COMMANDS: [&str; 2] = ["build", "plan"];
/// The command a location's plan preview runs, whose flags pinned jobs' build-wide settings become.
pub const PLAN_COMMAND: &str = "plan";
/// A SQL unit test's published kind; a frozen run selects it back with `test:<name>`.
pub const SQL_TEST_NODE_KIND: &str = "sql_test";
/// The cursor settings a frozen run's window replaces.
pub const WINDOW_CURSOR_SETTINGS: [&str; 6] = [
    "start_cursor",
    "end_cursor",
    "start_cursor_ts",
    "end_cursor_ts",
    "start_cursor_int",
    "end_cursor_int",
];
