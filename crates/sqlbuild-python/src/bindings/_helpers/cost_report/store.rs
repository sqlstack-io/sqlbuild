//! Python error translation for the shared native cost artifact store.

use std::path::{Path, PathBuf};

use pyo3::prelude::{PyErr, PyResult, Python};
use sqlbuild_cli::cost_store::errors::CostStoreError;
use sqlbuild_cli::cost_store::main::list_cost_runs::list_cost_runs;
use sqlbuild_cli::cost_store::main::read_cost_run::read_cost_run;
use sqlbuild_cli::cost_store::main::write_cost_run::write_cost_run;
use sqlbuild_core::cost_report::models::CostRun;

use crate::bindings::_helpers::cost_report::records::artifact_error;

fn failure(py: Python<'_>, error: CostStoreError) -> PyErr {
    match error {
        CostStoreError::Io(error) => PyErr::from(error),
        CostStoreError::Artifact(error) => artifact_error(py, &error),
    }
}

/// Atomically persist one run using the same implementation as the native launcher.
pub(crate) fn write(py: Python<'_>, project_dir: &Path, run: &CostRun) -> PyResult<PathBuf> {
    write_cost_run(project_dir, run).map_err(|error| failure(py, error))
}

/// Read one exact artifact, preserving Python's artifact versus I/O error distinction.
pub(crate) fn read(py: Python<'_>, project_dir: &Path, run_id: &str) -> PyResult<Option<CostRun>> {
    read_cost_run(project_dir, run_id).map_err(|error| failure(py, error))
}

/// The shared store skips unreadable artifacts and orders the rest newest first.
pub(crate) fn list(_py: Python<'_>, project_dir: &Path) -> PyResult<Vec<CostRun>> {
    Ok(list_cost_runs(project_dir))
}
