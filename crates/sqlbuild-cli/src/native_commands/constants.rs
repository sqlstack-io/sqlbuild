//! Names and limits of the Python-free command path.

/// `type=` converters the native parse applies itself; any other needs Python.
pub(crate) const INT_CONVERTER: &str = "int";

pub(crate) const POSITIVE_INTEGER_CONVERTER: &str = "_positive_integer";

pub(crate) const POSITIVE_INTEGER_MESSAGE: &str = "value must be a positive integer";

pub(crate) const NO_COLOR_OPTION: &str = "--no-color";

pub(crate) const NO_COLOR_VARIABLE: &str = "NO_COLOR";

/// Set by CI systems; `sqb dev` then prints plain lines.
pub(crate) const CI_VARIABLE: &str = "CI";

/// Turns live progress off; `sqb dev` then prints plain lines.
pub(crate) const NO_PROGRESS_VARIABLE: &str = "SQLBUILD_NO_PROGRESS";
/// Names the terminal type; `dumb` cannot move the cursor.
pub(crate) const TERM_VARIABLE: &str = "TERM";
/// The `TERM` of a terminal without cursor movement.
pub(crate) const DUMB_TERMINAL: &str = "dumb";

pub(crate) const COLUMNS_VARIABLE: &str = "COLUMNS";

/// `shutil.get_terminal_size`'s fallback width.
pub(crate) const FALLBACK_COLUMNS: i64 = 80;
/// The height a screen assumes when the terminal does not say.
pub(crate) const FALLBACK_ROWS: usize = 24;
/// The code of a rejected project configuration, as `ProjectConfigError` reports it.
pub(crate) const CONFIG_ERROR_CODE: &str = "D001";

/// Variables that change where Python imports `sqlbuild` from.
pub(crate) const PYTHON_PATH_VARIABLES: [&str; 2] = ["PYTHONPATH", "PYTHONHOME"];

pub(crate) const PACKAGE_NAME: &str = "sqlbuild";

pub(crate) const PACKAGE_MARKER: &str = "__init__.py";

pub(crate) const PATH_FILE_EXTENSION: &str = "pth";

/// `.pth` lines that `site` executes instead of adding to the search path.
pub(crate) const PATH_FILE_IMPORT_PREFIXES: [&str; 2] = ["import ", "import\t"];

pub(crate) const PATH_FILE_COMMENT: char = '#';

#[cfg(not(windows))]
pub(crate) const LIBRARY_DIRECTORY: &str = "lib";

#[cfg(not(windows))]
pub(crate) const INTERPRETER_DIRECTORY_PREFIX: &str = "python3";

#[cfg(not(windows))]
pub(crate) const SITE_DIRECTORIES: [&str; 2] = ["site-packages", "dist-packages"];

#[cfg(windows)]
pub(crate) const WINDOWS_SITE_DIRECTORY: [&str; 2] = ["Lib", "site-packages"];

/// The packaged skill beneath the `sqlbuild` package directory.
pub(crate) const SKILLS_DIRECTORY: [&str; 3] = [".agents", "skills", "sqlbuild"];

/// The variable `Path.home()` reads first; without it `sqb` defers to Python's other lookups.
#[cfg(not(windows))]
pub(crate) const HOME_VARIABLE: &str = "HOME";

#[cfg(windows)]
pub(crate) const HOME_VARIABLE: &str = "USERPROFILE";

/// The command name compute logs record for a bare `sqb`.
pub(crate) const UNKNOWN_COMMAND: &str = "unknown";

/// The supported Python ISO grammar, independent of whether a Python process is started.
pub(crate) const COST_ISO_PYTHON_MINOR: u8 = 12;

pub(crate) const RUNNING_BUILD_STATUS: &str = "running";

/// Commands with no handler: observed, then successful.
pub(crate) const CLEAN_COMMAND: &str = "clean";

pub(crate) const INIT_COMMAND: &str = "init";

pub(crate) const SKILLS_COMMAND: &str = "skills";

pub(crate) const SERVER_COMMAND: &str = "server";

pub(crate) const SCHEDULER_COMMAND: &str = "scheduler";

pub(crate) const ADMIN_COMMAND: &str = "admin";

/// How long `sqb admin` opens its store in silence; a migration or a busy store takes longer.
pub(crate) const ADMIN_OPEN_NOTICE: std::time::Duration = std::time::Duration::from_secs(1);
/// Printed when opening the control store starts.
pub(crate) const OPENING_STORE: &str = "Opening control store...\n";
/// Printed when the control store is open.
pub(crate) const STORE_READY: &str = "Control store ready\n";
/// An admin command committed, and its result didn't reach stdout.
pub(crate) const ADMIN_DELIVERY_FAILED: &str =
    "Control administration result delivery failed; changes may already be committed\n";
/// How a table shows a value that is absent.
pub(crate) const NO_VALUE: &str = "-";

pub(crate) const RULES_COMMAND: &str = "rules";

pub(crate) const RULES_LIST_ACTION: &str = "list";

pub(crate) const RULES_SHOW_ACTION: &str = "show";

/// Project directories holding Python the command would import.
pub(crate) const EVENT_EXPORTERS_DIRECTORY: &str = "event_exporters";

pub(crate) const SINKS_DIRECTORY: &str = "sinks";

pub(crate) const CUSTOM_RULES_DIRECTORY: &str = "rules";

pub(crate) const SKILL_FILES_HEADER: &str = "Updated SQLBuild skill files:\n";

pub(crate) const RULE_SELECTOR_REQUIRED: &str = "A rule code or family prefix is required";

/// `run_rules_command`'s exit status for a missing or unknown rule.
pub(crate) const RULES_USAGE_EXIT_CODE: i32 = 2;

/// The class Python's last-resort log handler names for a user error.
pub(crate) const USER_ERROR_CLASS: &str = "sqlbuild._native.CliUserError";

pub(crate) const INIT_FAILURE_PREFIX: &str = "sqb init: ";

/// A built-in rule's family is its code without the trailing three-digit number.
pub(crate) const FAMILY_SUFFIX_LENGTH: usize = 3;

/// `RuleSubject` values.
pub(crate) const MODEL_SUBJECT: &str = "model";

pub(crate) const PROJECT_SUBJECT: &str = "project";

/// Python `repr` of the built-in option defaults: `False` and `()`.
pub(crate) const FALSE_REPR: &str = "False";

pub(crate) const EMPTY_TUPLE_REPR: &str = "()";

/// The Python type name of each `allowed_qualifiers`-style table option item.
pub(crate) const DICT_TYPE_NAME: &str = "dict";

pub(crate) const UNKNOWN_RULE_PREFIX: &str = "Unknown rule: ";

/// The whitespace `int()` strips once other spaces are ASCII spaces.
pub(crate) const INT_ASCII_SPACES: &str = " \t\n\x0b\x0c\r";

pub(crate) const NEGATIVE_SIGN: char = '-';

pub(crate) const POSITIVE_SIGN: char = '+';

pub(crate) const ZERO_TEXT: &str = "0";

/// `sys.int_info.default_max_str_digits`: longer decimal text makes `int()` raise.
pub(crate) const PYTHON_INT_MAX_STR_DIGITS: usize = 4300;

pub(crate) const DIGIT_SEPARATOR: char = '_';

/// Where a loaded project configuration names its lifecycle sinks.
pub(crate) const LIFECYCLE_SINK_PATH: [&str; 3] = ["sinks", "lifecycle", "named"];

/// CPython's default buffer size for a piped stdout.
pub(crate) const PYTHON_STDOUT_BUFFER_SIZE: usize = 8192;

pub(crate) const BROKEN_PIPE_EXCEPTION: &str = "BrokenPipeError: [Errno 32] Broken pipe";

/// What Python prints when the final flush of a lost stdout fails, and the status it exits with.
pub(crate) const EXIT_FLUSH_FAILURE: &str =
    "Exception ignored in: <_io.TextIOWrapper name='<stdout>' mode='w' encoding='utf-8'>";

pub(crate) const EXIT_FLUSH_FAILURE_STATUS: i32 = 120;

pub(crate) const UNCAUGHT_EXCEPTION_STATUS: i32 = 1;

/// The largest answer a sensor's interpreter may send back.
pub(crate) const SENSOR_RESULT_BYTES: u64 = 1_048_576;

/// Read timeout, and the delay before retrying a failed page of an unfinished synchronization.
pub(crate) const METADATA_POLL_SECONDS: u64 = 30;

/// Failed reads in a row, waiting twice as long after each, that stop a sync until one is requested.
pub(crate) const METADATA_FAILURE_LIMIT: u32 = 5;

/// Characters of a failed read's error that its health report carries.
pub(crate) const METADATA_ERROR_CHARS: usize = 500;

/// A metadata reader that ignores end of input this long after a synchronization is killed.
pub(crate) const METADATA_READER_CLOSE_SECONDS: u64 = 5;

/// How often a location asks git whether `HEAD` moved.
pub(crate) const LOCATION_COMMIT_CHECK_SECONDS: u64 = 2;

/// How often an idle location asks the server, never the warehouse, whether a run ended.
pub(crate) const METADATA_SYNC_CHECK_SECONDS: u64 = 5;

/// Full key-set reconciliation is independent of the incremental poll and overlap window.
pub(crate) const METADATA_RECONCILE_SECONDS: u32 = 300;

/// An idle location syncs at least this often, so a lost run or request signal cannot leave drift.
pub(crate) const METADATA_SAFETY_SYNC_SECONDS: u64 = 1800;

/// Overrides `[control] sync_at_least_every`; `--sync-at-least-every` overrides both.
pub(crate) const SAFETY_SYNC_ENV: &str = "SQLBUILD_CONTROL_SYNC_AT_LEAST_EVERY";

pub(crate) const METADATA_OVERLAP_SECONDS: i64 = 300;

/// A source-key cursor can repeat the last row's base64-encoded definition beside the records.
pub(crate) const METADATA_REPLY_BYTES: u64 =
    3 * sqlbuild_orchestration::api::constants::BODY_BYTES as u64;

/// Bound piped terminal input independently of the credential owner's password policy.
pub(crate) const PASSWORD_INPUT_BYTES: u64 = 4096;

/// The command a location runs to answer a plan request.
pub(crate) const PLAN_COMMAND: &str = "plan";

/// How much `sqb plan --json` output a location reads; model diffs over the server's cap are dropped.
pub(crate) const PLAN_OUTPUT_BYTES: usize =
    8 * sqlbuild_orchestration::platform::constants::PLAN_DOCUMENT_BYTES;

/// `sqb dev` names at most this many changed inputs per change event.
pub(crate) const CHANGED_PATHS_SHOWN: usize = 3;

/// Seconds a state-table or catalog page may take; its byte budget bounds the transfer.
pub(crate) const METADATA_PAGE_SECONDS: u64 = 120;

/// Seconds added to a statistics page's allowance for each relation it samples.
pub(crate) const STATISTICS_RELATION_SECONDS: u64 = 1;

/// The command behind the web UI's "Build what changed": `sqb build --changed` with the project's default options.
pub(crate) const CHANGED_BUILD_COMMAND: [&str; 2] = ["build", "--changed"];

/// How long an idle location waits between checks for named work; after work it checks at once.
pub(crate) const LOCATION_WORK_IDLE_POLL: std::time::Duration = std::time::Duration::from_secs(2);

/// Finished plans a location keeps for identical requests; a newer generation or synced copy drops them.
pub(crate) const REUSED_PLANS: usize = 16;

/// The most bytes of plan text a location keeps for reuse; the oldest plans go first.
pub(crate) const REUSED_PLAN_BYTES: usize = 256 * 1024 * 1024;

/// How long a kept plan may answer a request: its windows and expiry follow the time it planned at.
pub(crate) const REUSED_PLAN_AGE: chrono::TimeDelta = chrono::TimeDelta::minutes(10);

/// What a location reports when `sqb plan` produced no plan.
pub(crate) const PLAN_NOT_PRODUCED: &str =
    "sqb plan did not produce a plan; see the location's log";

/// Return to the start of the line and erase it: a child's spinner may be drawn there.
pub(crate) const CLEAR_TERMINAL_LINE: &str = "\r\u{1b}[K";

/// The setting of `--changed`, which makes a plan read the synced copy's consumption.
pub(crate) const CHANGED_SETTING: &str = "changed";

/// What a second signal during worker shutdown prints before it kills the workers.
pub(crate) const FORCED_WORKER_STOP: &str =
    "Stopping now: killing the workers' process groups instead of waiting for them\n";

/// The command `sqb sensor` answers.
pub(crate) const SENSOR_COMMAND: &str = "sensor";
/// Exit status of a misused command, as for other usage errors.
pub(crate) const SENSOR_USAGE_EXIT: i32 = 2;
/// Exit status when the function failed, timed out, or there was nothing to call it with.
pub(crate) const SENSOR_FAILED_EXIT: i32 = 1;

/// The environment variable `sqb location republish` reads an API key from; never an argument.
pub(crate) const API_KEY_VARIABLE: &str = "SQLBUILD_API_KEY";
/// How long one `--wait` poll holds its response open, inside both clients' request timeouts.
pub(crate) const REPUBLISH_POLL_SECONDS: u64 = 10;
/// How long `sqb location republish --wait` waits unless told otherwise.
pub(crate) const REPUBLISH_TIMEOUT_SECONDS: u64 = 600;
