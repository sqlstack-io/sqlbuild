//! One run's recorder of the runs table, the same for the CLI and a worker.

use crate::native_host::_helpers::runs_state::run_identity;
use crate::native_host::models::{RunCommandScope, RunIdentity, RunRecorder};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::constants::RUNS_TABLE_NAME;
use sqlbuild_python_base::native_host::main::write_console::write_console;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_state::runs::models::ServerRun;
use std::path::PathBuf;

/// One run's recorder, the same for the CLI and a worker (which passes `server`); failures only warn.
pub fn run_recorder<'py>(
    (command, sqlbuild_version): (&'static str, &'static str),
    (request, invocation, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    server: Option<ServerRun>,
) -> Option<RunRecorder<'py>> {
    let opened = || -> PyResult<Option<RunRecorder<'py>>> {
        let project_dir: PathBuf = native_field(invocation, "effective_project_dir")?.extract()?;
        let settings: Bound<'py, PyAny> = native_field(
            &native_field(
                &native_field(invocation, "discovered_inputs")?,
                "project_config",
            )?,
            "settings",
        )?;
        let scope = RunCommandScope {
            command,
            project_dir: &project_dir,
            record_host: native_field(&settings, "record_run_host")?.is_truthy()?,
            sqlbuild_version,
        };
        let identity: RunIdentity = run_identity(
            request,
            &scope,
            (
                native_field(project, "run_id")?.str()?.extract()?,
                native_field(project, "effective_target_name")?.extract()?,
            ),
            server,
        )?;
        RunRecorder::open(
            (
                &native_field(invocation, "adapter")?,
                &native_field(invocation, "connection_config")?,
            ),
            (
                native_field(project, "effective_target_database")?.extract()?,
                native_field(project, "effective_target_schema")?.extract()?,
            ),
            identity,
        )
    };
    opened().unwrap_or_else(|error| {
        let _ = write_console(
            request.py(),
            "stderr",
            &format!("Warning: this run is not recorded in {RUNS_TABLE_NAME}: {error}\n"),
        );
        None
    })
}
