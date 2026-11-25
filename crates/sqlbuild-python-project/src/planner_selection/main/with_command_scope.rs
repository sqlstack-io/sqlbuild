//! Run one parsed command with its jobs selectable and, for commands that read them, where its run selectors read past runs.

use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_cli::run_selectors::main::past_run_source::past_run_source;
use sqlbuild_cli::run_selectors::models::PastRunSource;
use sqlbuild_recall::settings::models::{RecallSettings, StoreLocation};

use crate::job_settings::main::with_job_scope::with_job_scope;
use crate::planner_selection::_helpers::past_runs::with_past_run_source;
use crate::planner_selection::constants::RUN_SELECTOR_COMMANDS;

/// Run `run` with jobs from `project_dir` on `args`' `--target`, and past runs from its `--server`, else the local SQLite Recall store `recall` names.
pub fn with_command_scope<T>(
    args: &Bound<'_, PyAny>,
    (project_dir, recall): (PathBuf, &Option<(RecallSettings, PathBuf)>),
    run: impl FnOnce() -> T,
) -> PyResult<T> {
    let selected_target: Option<String> = attribute_text(args, "target")?;
    let source = command_source(args, (&project_dir, selected_target.clone()), recall)?;
    Ok(with_job_scope((project_dir, selected_target), || {
        with_past_run_source(source, run)
    }))
}

/// Where `args`' command reads past runs; `None` for a command that selects neither audits nor past runs.
fn command_source(
    args: &Bound<'_, PyAny>,
    (project_dir, selected_target): (&std::path::Path, Option<String>),
    recall: &Option<(RecallSettings, PathBuf)>,
) -> PyResult<Option<PastRunSource>> {
    let text = |name: &str| attribute_text(args, name);
    if !text("command")?.is_some_and(|command| RUN_SELECTOR_COMMANDS.contains(&command.as_str())) {
        return Ok(None);
    }
    let store = match recall.as_ref().map(|(settings, _)| &settings.store) {
        Some(StoreLocation::Sqlite { path }) => Some(path.clone()),
        _ => None,
    };
    Ok(Some(past_run_source(
        (
            text("login_server")?,
            text("login_ca")?.map(PathBuf::from),
            text("person_location")?,
        ),
        (project_dir, selected_target),
        store,
    )))
}

/// `args.<name>` as text; `None` when unset or missing.
fn attribute_text(args: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    match args.getattr_opt(name)? {
        Some(value) if !value.is_none() => Ok(Some(value.str()?.to_string())),
        _ => Ok(None),
    }
}
