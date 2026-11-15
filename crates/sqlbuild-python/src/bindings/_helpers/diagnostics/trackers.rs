//! Invocation-scoped partial build phase timings and process resource deltas.

use std::sync::{Mutex, PoisonError};

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyType};
use pyo3::{pyclass, pymethods};
use sqlbuild_host::entries::models::ExecutorHost;

use sqlbuild_python_base::boundary::main::context_variable::context_variable;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;

const TRACKER_VARIABLE: &str = "sqlbuild_build_phase_timing_tracker";
const PHASES: [&str; 6] = [
    "compile_seconds",
    "planning_seconds",
    "connection_preparation_seconds",
    "schema_preparation_seconds",
    "execution_seconds",
    "cost_collection_seconds",
];

static CURRENT_TRACKER: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// The context variable holding the active build timing tracker.
fn current_variable(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    context_variable(py, &CURRENT_TRACKER, TRACKER_VARIABLE)
}

/// The active build timing tracker, or None.
pub(crate) fn current_tracker(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    current_variable(py)?.call_method0("get")
}

/// `monotonic`, or `time.monotonic` when none is given.
fn clock<'py>(
    py: Python<'py>,
    monotonic: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    match monotonic {
        Some(monotonic) => Ok(monotonic),
        None => imported(py, "time", "monotonic"),
    }
}

/// `max(0.0, later - earlier)`.
fn non_negative(later: f64, earlier: f64) -> f64 {
    (later - earlier).max(0.0)
}

/// Retain disjoint monotonic phase durations across exceptional exits.
#[pyclass(module = "sqlbuild._native")]
pub(crate) struct BuildPhaseTimingTracker {
    monotonic: Py<PyAny>,
    started_at: f64,
    #[pyo3(get, set)]
    pub(crate) compile_seconds: Option<f64>,
    #[pyo3(get, set)]
    pub(crate) planning_seconds: Option<f64>,
    #[pyo3(get, set)]
    pub(crate) connection_preparation_seconds: Option<f64>,
    #[pyo3(get, set)]
    pub(crate) schema_preparation_seconds: Option<f64>,
    #[pyo3(get, set)]
    pub(crate) execution_seconds: Option<f64>,
    #[pyo3(get, set)]
    pub(crate) cost_collection_seconds: Option<f64>,
    tokens: Mutex<Vec<Py<PyAny>>>,
}

#[pymethods]
impl BuildPhaseTimingTracker {
    #[new]
    #[pyo3(signature = (*, monotonic = None))]
    pub(crate) fn new(py: Python<'_>, monotonic: Option<Bound<'_, PyAny>>) -> PyResult<Self> {
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
    fn snapshot<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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
    fn current<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyAny>> {
        current_variable(cls.py())?.call_method0("get")
    }

    /// Install this tracker for one command invocation and reset it afterward.
    fn scope(slf: &Bound<'_, Self>) -> PhaseTimingScope {
        PhaseTimingScope {
            tracker: slf.clone().unbind(),
        }
    }
}

impl BuildPhaseTimingTracker {
    fn tokens(&self) -> std::sync::MutexGuard<'_, Vec<Py<PyAny>>> {
        self.tokens.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Makes one tracker the active build timing tracker while entered.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct PhaseTimingScope {
    tracker: Py<BuildPhaseTimingTracker>,
}

#[pymethods]
impl PhaseTimingScope {
    fn __enter__(&self, py: Python<'_>) -> PyResult<()> {
        let token: Bound<'_, PyAny> =
            current_variable(py)?.call_method1("set", (self.tracker.bind(py),))?;
        self.tracker.bind(py).borrow().tokens().push(token.unbind());
        Ok(())
    }

    #[pyo3(signature = (*_exception))]
    fn __exit__(&self, py: Python<'_>, _exception: &Bound<'_, PyTuple>) -> PyResult<bool> {
        let token: Option<Py<PyAny>> = self.tracker.bind(py).borrow().tokens().pop();
        if let Some(token) = token {
            current_variable(py)?.call_method1("reset", (token,))?;
        }
        Ok(false)
    }
}

/// Capture wall and process resource deltas around one command.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct ProcessResourceTracker {
    monotonic: Py<PyAny>,
    resource_reader: Py<PyAny>,
    started_at: f64,
    started_user: f64,
    started_system: f64,
}

#[pymethods]
impl ProcessResourceTracker {
    #[new]
    #[pyo3(signature = (*, monotonic = None, resource_reader = None))]
    fn new(
        py: Python<'_>,
        monotonic: Option<Bound<'_, PyAny>>,
        resource_reader: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let monotonic: Bound<'_, PyAny> = clock(py, monotonic)?;
        let resource_reader: Bound<'_, PyAny> = match resource_reader {
            Some(reader) => reader,
            None => host_entry(py, ExecutorHost::ReadProcessResources)?,
        };
        let started_at: f64 = monotonic.call0()?.extract()?;
        let (started_user, started_system, _): (f64, f64, Bound<'_, PyAny>) =
            resource_reader.call0()?.extract()?;
        Ok(Self {
            monotonic: monotonic.unbind(),
            resource_reader: resource_reader.unbind(),
            started_at,
            started_user,
            started_system,
        })
    }

    /// Return elapsed wall/CPU deltas and process peak RSS.
    fn finish<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let (user_cpu, system_cpu, max_rss_bytes): (f64, f64, Bound<'py, PyAny>) =
            self.resource_reader.bind(py).call0()?.extract()?;
        let now: f64 = self.monotonic.bind(py).call0()?.extract()?;
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        keywords.set_item("wall_seconds", non_negative(now, self.started_at))?;
        keywords.set_item(
            "user_cpu_seconds",
            non_negative(user_cpu, self.started_user),
        )?;
        keywords.set_item(
            "system_cpu_seconds",
            non_negative(system_cpu, self.started_system),
        )?;
        keywords.set_item("max_rss_bytes", max_rss_bytes)?;
        host_entry(py, ExecutorHost::ProcessResourceUsage)?.call((), Some(&keywords))
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<BuildPhaseTimingTracker>()?;
    module.add_class::<PhaseTimingScope>()?;
    module.add_class::<ProcessResourceTracker>()
}
