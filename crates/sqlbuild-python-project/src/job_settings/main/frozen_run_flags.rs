//! The command-line flags of a frozen run's settings.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_cli::argv::main::command_settings_arguments::command_settings_arguments;
use sqlbuild_cli::argv::main::job_run_arguments::job_run_arguments;
use sqlbuild_cli::argv::main::job_settings_arguments::job_settings_arguments;
use sqlbuild_orchestration::runs::models::{RunCommand, RunRequest, RunWindow};

use crate::job_settings::_helpers::job_run_settings::job_run_settings;
use crate::job_settings::constants::WINDOW_CURSOR_SETTINGS;
use crate::job_settings::models::FrozenRunFlags;

/// A frozen run's settings with its window's cursors, and the flags a worker passes for them.
pub fn frozen_run_flags(
    frozen: &RunRequest,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<FrozenRunFlags> {
    let mut settings = frozen.settings.clone();
    if let Some(window) = &frozen.window {
        for key in WINDOW_CURSOR_SETTINGS {
            settings.remove(key);
        }
        let (kind, start, end) = match window {
            RunWindow::Timestamp { start, end } => ("ts", start.to_rfc3339(), end.to_rfc3339()),
            RunWindow::Integer { start, end } => ("int", start.to_string(), end.to_string()),
        };
        settings.insert(format!("start_cursor_{kind}"), serde_json::json!(start));
        settings.insert(format!("end_cursor_{kind}"), serde_json::json!(end));
    }
    if frozen.job_key.is_some() {
        settings = job_run_settings(project_dir, settings)?;
    }
    let job_build: Option<String> = frozen
        .job_key
        .clone()
        .filter(|_| frozen.command == RunCommand::Build);
    let flags: Vec<String> = match &job_build {
        Some(job) => job_run_arguments(job, &settings),
        None if frozen.command == RunCommand::Build => job_settings_arguments(&settings),
        None => command_settings_arguments(frozen.command.as_str(), &settings),
    }
    .map_err(PyValueError::new_err)?;
    Ok(FrozenRunFlags {
        settings,
        flags,
        job_build,
    })
}
