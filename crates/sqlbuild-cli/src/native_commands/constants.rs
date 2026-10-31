//! Names and limits of the Python-free command path.

/// `type=` converters the native parse applies itself; any other needs Python.
pub(crate) const INT_CONVERTER: &str = "int";
pub(crate) const POSITIVE_INTEGER_CONVERTER: &str = "_positive_integer";
pub(crate) const POSITIVE_INTEGER_MESSAGE: &str = "value must be a positive integer";
pub(crate) const NO_COLOR_OPTION: &str = "--no-color";
pub(crate) const NO_COLOR_VARIABLE: &str = "NO_COLOR";
pub(crate) const COLUMNS_VARIABLE: &str = "COLUMNS";
/// `shutil.get_terminal_size`'s fallback width.
pub(crate) const FALLBACK_COLUMNS: i64 = 80;
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
pub(crate) const LEGACY_PROJECT_CONFIG_FILENAME: &str = "sqlbuild_project.yml";
pub(crate) const LOCAL_CONFIG_FILENAME: &str = "sqlbuild_local.toml";
pub(crate) const LEGACY_LOCAL_CONFIG_FILENAME: &str = "sqlbuild_local.yml";
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
