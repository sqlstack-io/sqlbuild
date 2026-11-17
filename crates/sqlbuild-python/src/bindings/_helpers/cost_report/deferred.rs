//! A worker build's warehouse costs, collected after the run reports its outcome so the run is not held open.

use pyo3::prelude::{PyResult, Python};

use crate::bindings::_helpers::cost_report::degradation::finalize_build_cost_within;
use sqlbuild_python_output::recall::main::write_stderr::write_stderr;
use sqlbuild_python_project::project_snapshot::models::WorkerBuild;

/// Collect a deferred build's costs after its run finished; a failure only warns, as the outcome is published.
pub(crate) fn collect_deferred_cost_after_finish(py: Python<'_>, worker: &WorkerBuild) {
    if let Err(error) = collect_deferred_cost(py, worker) {
        let _ = write_stderr(
            py,
            &format!("warning: warehouse cost collection failed: {error}\n"),
        );
    }
}

fn collect_deferred_cost(py: Python<'_>, worker: &WorkerBuild) -> PyResult<()> {
    let Some(finalization) = worker.take_deferred_cost()? else {
        return Ok(());
    };
    write_stderr(py, "Collecting warehouse costs...\n")?;
    finalize_build_cost_within(finalization.bind(py), None).map(|_| ())
}
