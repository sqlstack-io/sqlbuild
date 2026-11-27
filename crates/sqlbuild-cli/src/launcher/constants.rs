//! Names the launcher shares with the Python CLI and its packaging.

/// The console script the `sqlbuild` wheel installs beside `sqb`; it runs the Python CLI.
#[cfg(not(windows))]
pub const PYTHON_CLI_SCRIPT: &str = "sqlbuild-python";
/// The console script the `sqlbuild` wheel installs beside `sqb`; it runs the Python CLI.
#[cfg(windows)]
pub const PYTHON_CLI_SCRIPT: &str = "sqlbuild-python.exe";
/// The first argument naming the launch record the Python CLI may write for this invocation.
pub const LAUNCH_RECORD_OPTION: &str = "--sqlbuild-launch-record=";
/// The program name argparse reports, fixed by the Python parser.
pub const PROGRAM_NAME: &str = "sqb";
pub const VERSION_OPTIONS: [&str; 2] = ["--version", "-V"];
/// The version `--version` prints, alone or among other options; the wheels share it.
pub const PROGRAM_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Global options naming the project directory, exactly as the Python parser spells them.
pub const PROJECT_DIR_OPTIONS: [&str; 2] = ["--project-dir", "--sqb-project-dir"];
/// Prefixes argparse could expand to a project directory option.
pub const PROJECT_DIR_ABBREVIATION_PREFIXES: [&str; 2] = ["--p", "--sq"];
/// Variables whose value changes between otherwise identical shell commands.
pub const VOLATILE_ENVIRONMENT: [&str; 4] = ["OLDPWD", "PWD", "SHLVL", "_"];
/// Domain separator of the launch key digest; bump when its inputs change.
pub const LAUNCH_KEY_DOMAIN: &str = "sqlbuild-launch-key-1";
/// The launch key's stamp of a file that does not exist.
pub const MISSING_FILE_STAMP: &[u8] = b"missing";
/// mimalloc's purge delay in milliseconds for the Python CLI, unless the user sets one.
pub const ALLOCATOR_PURGE_DELAY: (&str, &str) = ("MIMALLOC_PURGE_DELAY", "100");
/// Exit code when the Python CLI cannot be started.
pub const HANDOFF_FAILURE_EXIT_CODE: i32 = 1;
