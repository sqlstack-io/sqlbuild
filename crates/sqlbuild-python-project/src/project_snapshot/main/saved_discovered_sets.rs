//! The discovered sets a claimed run's snapshot saved, read before the worker compiles.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_orchestration::discovered_sets::types::DiscoveredSets;
use sqlbuild_orchestration::worker::main::connect_worker::connect_worker;
use sqlbuild_orchestration::worker::main::fetch_execution::fetch_execution;
use uuid::Uuid;

/// The sets the server saved for a claimed run, so the worker compiles the published nodes.
pub fn saved_discovered_sets(
    py: Python<'_>,
    (server_url, bearer): (&str, &str),
    (run_id, claim_id): (&str, &str),
) -> PyResult<DiscoveredSets> {
    let identity = (
        Uuid::parse_str(run_id).map_err(|error| PyRuntimeError::new_err(error.to_string()))?,
        Uuid::parse_str(claim_id).map_err(|error| PyRuntimeError::new_err(error.to_string()))?,
    );
    py.detach(|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        runtime
            .block_on(async {
                let client = connect_worker(server_url, bearer, identity)?;
                fetch_execution(&client).await
            })
            .map(|execution| execution.discovered)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    })
}
