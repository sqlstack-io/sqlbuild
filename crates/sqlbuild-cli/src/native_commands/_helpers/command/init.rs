//! `sqb init`: scaffold a blank project and install its skills.

use std::path::{Path, PathBuf};

use sqlbuild_config::workspace::errors::WorkspaceError;
use sqlbuild_config::workspace::main::install_skills::install_skills;
use sqlbuild_config::workspace::models::SkillInstallRequest;
use sqlbuild_core::cli_text::main::format_coded_error::format_coded_error;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_core::init_command::errors::ScaffoldError;
use sqlbuild_core::init_command::main::init_project_name::init_project_name;
use sqlbuild_core::init_command::main::init_project_toml::init_project_toml;
use sqlbuild_core::init_command::main::render_init_summary::render_init_summary;
use sqlbuild_core::init_command::main::scaffold_blank_project::scaffold_blank_project;

use crate::dispatch::main::failure_report::failure_report;
use crate::dispatch::models::CommandFailure;
use crate::native_commands::_helpers::command::skills::home_dir;
use crate::native_commands::_helpers::command::terminal::{
    error_description, stdout_supports_color,
};
use crate::native_commands::constants::{INIT_FAILURE_PREFIX, USER_ERROR_CLASS};
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::models::{CommandOutput, InstalledPackage};

/// A failure once the project file exists, which a Python rerun would report differently.
fn failed(stderr: String) -> Result<CommandOutput, PythonNeeded> {
    Ok(CommandOutput {
        stdout: String::new(),
        stderr,
        exit_code: 1,
    })
}

/// The project's skills could not be installed: what the Python CLI prints for its user error.
fn skills_failure(
    base_dir: &Path,
    error: &WorkspaceError,
    use_color: bool,
) -> Result<CommandOutput, PythonNeeded> {
    let (code, message, help) = match error {
        WorkspaceError::User {
            code,
            message,
            help,
        } => (code, message, help),
        WorkspaceError::Decode { path, .. } => {
            return failed(format!(
                "{INIT_FAILURE_PREFIX}cannot install SQLBuild skills: {} is not UTF-8 text\n",
                path.display()
            ));
        }
        WorkspaceError::Io(error) => {
            return failed(format!(
                "{INIT_FAILURE_PREFIX}cannot install SQLBuild skills into {}: {}\n",
                base_dir.display(),
                error_description(error)
            ));
        }
    };
    let (_, log_message) = failure_report(CommandFailure::User);
    let coded: String = format_coded_error(
        (code, message, help.as_deref()),
        true,
        CliStyle { use_color },
    );
    failed(format!(
        "{log_message}\n{USER_ERROR_CLASS}: {message}\n{coded}\n\n"
    ))
}

/// Create the project; a failure before the project file exists is Python's to report.
pub(crate) fn run_init(
    base_dir: &Path,
    package: &InstalledPackage,
    error_color: bool,
) -> Result<CommandOutput, PythonNeeded> {
    let project_file: PathBuf = base_dir.join(PROJECT_CONFIG_FILENAME);
    if project_file.exists() {
        return Err(PythonNeeded::Failure);
    }
    let directory_name: &str = match base_dir.file_name() {
        Some(name) => name.to_str().ok_or(PythonNeeded::Arguments)?,
        None => "",
    };
    let home: PathBuf = home_dir()?;
    let project_name: String = init_project_name(directory_name);
    if let Err(ScaffoldError { error, path }) =
        scaffold_blank_project(base_dir, &init_project_toml(&project_name))
    {
        if !project_file.exists() {
            return Err(PythonNeeded::Failure);
        }
        return failed(format!(
            "{INIT_FAILURE_PREFIX}cannot create {}: {}\n",
            path.display(),
            error_description(&error)
        ));
    }
    if let Err(error) = install_skills(&SkillInstallRequest {
        project_dir: base_dir.to_path_buf(),
        skills_root: package.skills_root.clone(),
        home_dir: home,
        global_install: false,
        requested_targets: Vec::new(),
        force: false,
    }) {
        return skills_failure(base_dir, &error, error_color);
    }
    Ok(CommandOutput {
        stdout: render_init_summary(
            &project_name,
            CliStyle {
                use_color: stdout_supports_color(),
            },
        ),
        ..CommandOutput::default()
    })
}
