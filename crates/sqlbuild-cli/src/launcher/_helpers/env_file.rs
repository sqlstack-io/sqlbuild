//! Load the project's env file once, before anything reads the environment or starts a child.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use sqlbuild_config::env_file::main::load_env_file::load_env_file;
use sqlbuild_core::process_environment::main::env_file_marker::env_file_marker;
use sqlbuild_core::process_environment::main::install_environment_overlay::install_environment_overlay;

use crate::argv::main::parse_cli_arguments::parse_cli_arguments;
use crate::argv::models::ArgvSettings;
use crate::native_commands::_helpers::command::argv_host::{NativeArgvHost, namespace_defaults};
use crate::native_commands::_helpers::command::decision::text;
use crate::native_commands::constants::FALLBACK_COLUMNS;
use crate::native_commands::main::effective_target::effective_target;

/// Exit code when the configured env file cannot be loaded, as for other configuration errors.
const ENV_FILE_FAILURE_EXIT_CODE: i32 = 1;

/// The project directory and `--target` the arguments name, `None` when they do not parse.
fn project_and_target(arguments: &[OsString]) -> Option<(PathBuf, Option<String>)> {
    let arguments: Vec<String> = arguments
        .iter()
        .map(|argument| argument.to_str().map(str::to_owned))
        .collect::<Option<_>>()?;
    let settings = ArgvSettings {
        columns: FALLBACK_COLUMNS,
        use_color: false,
    };
    let outcome = parse_cli_arguments(
        &arguments,
        namespace_defaults(),
        settings,
        &mut NativeArgvHost::default(),
    );
    let namespace = outcome.parsed.ok()?;
    let cwd: PathBuf = std::env::current_dir().ok()?;
    let project_dir: PathBuf =
        text(&namespace, "project_dir").map_or_else(|| cwd.clone(), |named| cwd.join(named));
    Some((project_dir, text(&namespace, "target")))
}

/// Whether this process inherited the load of the same project and target from its parent.
fn inherited(project_dir: &Path, target: Option<&str>) -> bool {
    let Some(marker) = env_file_marker() else {
        return false;
    };
    let canonical = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());
    Path::new(&marker.project_dir) == canonical && marker.target.as_deref() == target
}

/// Load the env file the project and target choose; `Err` is the exit code after reporting why not.
///
/// A parent's load is inherited, so a process tree reads the file once. Nothing happens outside
/// a project, or when the configuration is invalid, which the command itself reports.
pub(crate) fn load_project_env_file(arguments: &[OsString]) -> Result<(), i32> {
    let Some((project_dir, selected)) = project_and_target(arguments) else {
        return Ok(());
    };
    if !project_dir
        .join(sqlbuild_core::constants::PROJECT_CONFIG_FILENAME)
        .is_file()
    {
        return Ok(());
    }
    let Ok(target) = effective_target(&project_dir, selected) else {
        return Ok(());
    };
    if inherited(&project_dir, target.as_deref()) {
        return Ok(());
    }
    match load_env_file(&project_dir, target.as_deref()) {
        Ok(None) => Ok(()),
        Ok(Some(load)) => {
            if let Some(warning) = &load.warning {
                let _ = writeln!(std::io::stderr(), "warning: {warning}");
            }
            install_environment_overlay(load.overlay);
            Ok(())
        }
        Err(error) => {
            let _ = writeln!(
                std::io::stderr(),
                "error: {}\n  = help: {}",
                error.message(),
                error.help()
            );
            Err(ENV_FILE_FAILURE_EXIT_CODE)
        }
    }
}
