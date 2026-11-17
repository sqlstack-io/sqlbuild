//! Process-local statement-ledger failure tracking for the active build.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::PyTypeMethods;
use pyo3::{pyclass, pymethods};

use sqlbuild_python_front::cost_context::main::process_object::process_object;

pub(crate) const HEALTH_ATTRIBUTE: &str = "COST_TELEMETRY_HEALTH";

static HEALTH: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// The process-wide ledger failure tracker, exposed as `COST_TELEMETRY_HEALTH`.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct CostTelemetryHealth {
    ledger_failures: Mutex<HashMap<String, String>>,
}

impl CostTelemetryHealth {
    fn failures(&self) -> MutexGuard<'_, HashMap<String, String>> {
        self.ledger_failures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

#[pymethods]
impl CostTelemetryHealth {
    /// Remember the first ledger failure of `run_id` by its exception type name.
    #[pyo3(signature = (*, run_id, error))]
    fn mark_ledger_failure(&self, run_id: String, error: &Bound<'_, PyAny>) -> PyResult<()> {
        let name: String = error.get_type().name()?.extract()?;
        self.failures().entry(run_id).or_insert(name);
        Ok(())
    }

    /// The ledger failure recorded for `run_id`, forgetting it.
    #[pyo3(signature = (*, run_id))]
    fn consume_ledger_failure(&self, run_id: &str) -> Option<String> {
        self.failures().remove(run_id)
    }
}

pub(crate) fn health(py: Python<'_>) -> PyResult<Bound<'_, CostTelemetryHealth>> {
    let tracker: &Bound<'_, PyAny> = process_object(&HEALTH, py, || {
        Ok(Bound::new(
            py,
            CostTelemetryHealth {
                ledger_failures: Mutex::new(HashMap::new()),
            },
        )?
        .into_any())
    })?;
    Ok(tracker.cast::<CostTelemetryHealth>()?.clone())
}

/// Remember the first ledger failure of `run_id` by its exception type name.
pub(crate) fn mark_ledger_failure(run_id: String, error: &Bound<'_, PyAny>) -> PyResult<()> {
    health(error.py())?.get().mark_ledger_failure(run_id, error)
}

/// The ledger failure recorded for `run_id`, forgetting it.
pub(crate) fn consume_ledger_failure(py: Python<'_>, run_id: &str) -> PyResult<Option<String>> {
    Ok(health(py)?.get().consume_ledger_failure(run_id))
}
