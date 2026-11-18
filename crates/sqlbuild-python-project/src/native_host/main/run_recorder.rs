//! One run's recorder of the runs table, the same for the CLI and a worker.

use crate::native_host::_helpers::runs_state::run_identity;
use crate::native_host::models::{RunCommandScope, RunIdentity, RunRecorder};
use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::target_checks::constants::PRESERVE_TARGET_VALUE;
use sqlbuild_core::constants::RUNS_TABLE_NAME;
use sqlbuild_python_base::native_host::main::write_console::write_console;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_state::runs::models::ServerRun;
use std::path::PathBuf;

/// A run's recorder with its started row written, or none for a `preserve` schema; only an interrupt errors.
pub fn run_recorder<'py>(
    (command, sqlbuild_version): (&'static str, &'static str),
    (request, invocation, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    server: Option<ServerRun>,
) -> PyResult<Option<RunRecorder<'py>>> {
    let location = |value: Option<String>| value.filter(|value| value != PRESERVE_TARGET_VALUE);
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
                location(native_field(project, "effective_target_database")?.extract()?),
                location(native_field(project, "effective_target_schema")?.extract()?),
            ),
            identity,
        )
    };
    match opened() {
        Err(error) if error.is_instance_of::<PyException>(request.py()) => {
            let _ = write_console(
                request.py(),
                "stderr",
                &format!("Warning: this run is not recorded in {RUNS_TABLE_NAME}: {error}\n"),
            );
            Ok(None)
        }
        Ok(Some(recorder)) => {
            recorder.started()?;
            Ok(Some(recorder))
        }
        opened => opened,
    }
}
