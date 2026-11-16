//! Public records of diagnostics.

use crate::diagnostics::_helpers::trackers::PHASES;
use crate::diagnostics::_helpers::trackers::PhaseTimingScope;
use crate::diagnostics::_helpers::trackers::clock;
use crate::diagnostics::_helpers::trackers::current_variable;
use crate::diagnostics::_helpers::trackers::non_negative;
use crate::records::main::construct::new_record::new_record;
use crate::records::types::RecordId;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyType;
use std::sync::Mutex;
use std::sync::PoisonError;

/// Retain disjoint monotonic phase durations across exceptional exits.
#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub struct BuildPhaseTimingTracker {
    pub(crate) monotonic: Py<PyAny>,
    pub(crate) started_at: f64,
    #[pyo3(get, set)]
    pub compile_seconds: Option<f64>,
    #[pyo3(get, set)]
    pub planning_seconds: Option<f64>,
    #[pyo3(get, set)]
    pub connection_preparation_seconds: Option<f64>,
    #[pyo3(get, set)]
    pub schema_preparation_seconds: Option<f64>,
    #[pyo3(get, set)]
    pub execution_seconds: Option<f64>,
    #[pyo3(get, set)]
    pub cost_collection_seconds: Option<f64>,
    pub(crate) tokens: Mutex<Vec<Py<PyAny>>>,
}

#[pymethods]
impl BuildPhaseTimingTracker {
    #[new]
    #[pyo3(signature = (*, monotonic = None))]
    pub fn new(py: Python<'_>, monotonic: Option<Bound<'_, PyAny>>) -> PyResult<Self> {
        let monotonic: Bound<'_, PyAny> = clock(py, monotonic)?;
        let started_at: f64 = monotonic.call0()?.extract()?;
        Ok(Self {
            monotonic: monotonic.unbind(),
            started_at,
            compile_seconds: None,
            planning_seconds: None,
            connection_preparation_seconds: None,
            schema_preparation_seconds: None,
            execution_seconds: None,
            cost_collection_seconds: None,
            tokens: Mutex::new(Vec::new()),
        })
    }

    /// Return currently available phase durations and total wall time.
    pub(crate) fn snapshot<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let now: f64 = self.monotonic.bind(py).call0()?.extract()?;
        let values: [Option<f64>; 6] = [
            self.compile_seconds,
            self.planning_seconds,
            self.connection_preparation_seconds,
            self.schema_preparation_seconds,
            self.execution_seconds,
            self.cost_collection_seconds,
        ];
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        for (phase, value) in PHASES.into_iter().zip(values) {
            keywords.set_item(phase, value)?;
        }
        keywords.set_item("total_seconds", non_negative(now, self.started_at))?;
        new_record(py, RecordId::PartialBuildPhaseTimings, (), Some(&keywords))
    }

    /// Return the active build timing tracker.
    #[classmethod]
    pub(crate) fn current<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyAny>> {
        current_variable(cls.py())?.call_method0("get")
    }

    /// Install this tracker for one command invocation and reset it afterward.
    pub(crate) fn scope(slf: &Bound<'_, Self>) -> PhaseTimingScope {
        PhaseTimingScope {
            tracker: slf.clone().unbind(),
        }
    }
}

impl BuildPhaseTimingTracker {
    pub(crate) fn tokens(&self) -> std::sync::MutexGuard<'_, Vec<Py<PyAny>>> {
        self.tokens.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
