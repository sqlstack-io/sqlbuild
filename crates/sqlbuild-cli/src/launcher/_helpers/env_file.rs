//! Load the project's env file once, before anything reads the environment or starts a child.

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;

use sqlbuild_config::env_file::main::load_env_file::load_env_file;
use sqlbuild_core::process_environment::constants::{ENV_FILE_ERROR_VARIABLE, RUN_TOKEN_VARIABLE};
use sqlbuild_core::process_environment::main::env_file_marker::env_file_marker;
use sqlbuild_core::process_environment::main::environment_var_os::environment_var_os;
use sqlbuild_core::process_environment::main::install_environment_overlay::install_environment_overlay;
use sqlbuild_core::process_environment::models::EnvironmentOverlay;

use crate::argv::main::command_project_and_target::command_project_and_target;
use crate::native_commands::main::effective_target::effective_target;

/// Exit code when the configured env file cannot be loaded, as for other configuration errors.
const ENV_FILE_FAILURE_EXIT_CODE: i32 = 1;

/// The project directory and target the arguments name, `None` when they are not Unicode or do not parse.
fn command_line_project(arguments: &[OsString]) -> Option<(Option<String>, Option<String>)> {
    let mut texts: Vec<String> = Vec::new();
    for argument in arguments {
        texts.push(argument.to_str()?.to_owned());
    }
    command_project_and_target(&texts)
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

/// Load the env file the project and target choose once per process tree; `Err` is the exit code after reporting why not, except in a run worker, which records why for the run.
pub(crate) fn load_project_env_file(arguments: &[OsString]) -> Result<(), i32> {
    let Some((named_dir, selected)) = command_line_project(arguments) else {
        return Ok(());
    };
    let Ok(cwd) = std::env::current_dir() else {
        return Ok(());
    };
    let project_dir = named_dir.map_or_else(|| cwd.clone(), |named| cwd.join(named));
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
            if environment_var_os(RUN_TOKEN_VARIABLE).is_none() {
                return Err(ENV_FILE_FAILURE_EXIT_CODE);
            }
            install_environment_overlay(EnvironmentOverlay {
                set: vec![(
                    OsString::from(ENV_FILE_ERROR_VARIABLE),
                    OsString::from(error.message()),
                )],
                remove: Vec::new(),
            });
            Ok(())
        }
    }
}
