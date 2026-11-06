//! SQL test planning on a native thread, joined by the planner or, at interpreter exit, within a bound.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::sync::PyOnceLock;
use pyo3::types::PyDict;
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};

use crate::bindings::_helpers::boundary::panics::compiler_guard;
use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, imported};
use crate::bindings::_helpers::native_threads::exit_signal::signal_interpreter_exit;
use crate::bindings::_helpers::planner_entries::plan_output::plan_selected_sql_tests;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::ids::RecordId;

const TEST_PLANNING_THREAD: &str = "sqlbuild-test-planning";
/// How long interpreter exit waits for SQL test planning threads still running native code.
const PLANNING_EXIT_JOIN: Duration = Duration::from_secs(5);
/// How often a waiting planner wakes to notice interrupt signals.
const RESULT_POLL: Duration = Duration::from_millis(100);

/// How often interpreter exit checks whether planning threads finished.
const EXIT_POLL: Duration = Duration::from_millis(10);

/// The process-wide count of running planning threads.
static PLANNING_THREADS: PyOnceLock<Py<PlanningThreads>> = PyOnceLock::new();

/// Planning threads still running native code.
#[pyclass(module = "sqlbuild._native", frozen)]
struct PlanningThreads {
    running: AtomicUsize,
}

/// Marks one planning thread finished when dropped, including when the thread panics.
struct RunningPlanning(Py<PlanningThreads>);

impl Drop for RunningPlanning {
    fn drop(&mut self) {
        self.0.get().finished();
    }
}

impl PlanningThreads {
    fn started(&self) {
        self.running.fetch_add(1, Ordering::AcqRel);
    }

    fn finished(&self) {
        self.running.fetch_sub(1, Ordering::AcqRel);
    }

    /// Wait without the GIL until no planning thread runs or `limit` passes.
    fn wait(&self, limit: Duration) {
        let deadline: Instant = Instant::now() + limit;
        while self.running.load(Ordering::Acquire) > 0 && Instant::now() < deadline {
            std::thread::sleep(EXIT_POLL);
        }
    }
}

/// The running planning threads, created on first use.
fn planning_threads(py: Python<'_>) -> PyResult<&Py<PlanningThreads>> {
    PLANNING_THREADS.get_or_try_init(py, || {
        Py::new(
            py,
            PlanningThreads {
                running: AtomicUsize::new(0),
            },
        )
    })
}

/// The planned SQL tests, or the error planning raised.
type Planned = Result<Py<PyAny>, PyErr>;

/// The planned SQL tests of one plan, planned on a native thread when enabled.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct SqlTestPlanning {
    pending: Mutex<Option<Receiver<Planned>>>,
    finished: Mutex<Option<Planned>>,
}

impl SqlTestPlanning {
    fn of(pending: Option<Receiver<Planned>>, finished: Option<Planned>) -> Self {
        Self {
            pending: Mutex::new(pending),
            finished: Mutex::new(finished),
        }
    }

    /// The outcome the planning thread sends, waited for without the GIL or any lock held.
    fn received(&self, py: Python<'_>, mut receiver: Receiver<Planned>) -> PyResult<Planned> {
        loop {
            let (waited, next) = py.detach(move || {
                let next = receiver.recv_timeout(RESULT_POLL);
                (receiver, next)
            });
            receiver = waited;
            match next {
                Ok(planned) => return Ok(planned),
                Err(RecvTimeoutError::Timeout) => {
                    if let Err(signal) = py.check_signals() {
                        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) =
                            Some(receiver);
                        return Err(signal);
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Ok(Err(PyRuntimeError::new_err(
                        "SQL test planning thread stopped",
                    )));
                }
            }
        }
    }
}

#[pymethods]
impl SqlTestPlanning {
    /// Wait for planning to finish and return the planned tests, raising its error.
    pub(crate) fn result(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let pending: Option<Receiver<Planned>> = self
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(receiver) = pending {
            let planned: Planned = self.received(py, receiver)?;
            *self.finished.lock().unwrap_or_else(PoisonError::into_inner) = Some(planned);
        }
        match self
            .finished
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            Some(Ok(planned)) => Ok(planned.clone_ref(py)),
            Some(Err(error)) => Err(error.clone_ref(py)),
            None => Err(PyRuntimeError::new_err(
                "SQL test planning result is awaited elsewhere",
            )),
        }
    }
}

/// The inputs one planning run reads, owned so they can move to the planning thread.
pub(crate) struct PlanningRequest {
    pub(crate) project: Py<PyAny>,
    pub(crate) adapter: Py<PyAny>,
    pub(crate) selected_keys: Py<PyAny>,
    pub(crate) sql_test_selection: Py<PyAny>,
}

/// Plan the request's selected SQL tests, reporting a panic as a native compiler error.
fn plan(py: Python<'_>, request: &PlanningRequest) -> Planned {
    compiler_guard(|| {
        plan_selected_sql_tests(
            request.project.bind(py),
            request.adapter.bind(py),
            request.selected_keys.bind(py),
            request.sql_test_selection.bind(py),
        )
        .map(Bound::unbind)
    })
}

/// Plan on a native thread when `enabled`, or resolve at once to no planned tests.
pub(crate) fn start_planning(
    py: Python<'_>,
    request: PlanningRequest,
    enabled: bool,
) -> PyResult<Py<SqlTestPlanning>> {
    if !enabled {
        let empty = new_record_with(
            py,
            RecordId::PlannedSqlTests,
            &[
                ("selected_keys", request.selected_keys.bind(py).clone()),
                (
                    "sql_test_selection",
                    request.sql_test_selection.bind(py).clone(),
                ),
            ],
        )?;
        return Py::new(py, SqlTestPlanning::of(None, Some(Ok(empty.unbind()))));
    }
    let (sender, receiver): (Sender<Planned>, Receiver<Planned>) = channel();
    let threads: Py<PlanningThreads> = planning_threads(py)?.clone_ref(py);
    threads.get().started();
    let spawned = std::thread::Builder::new()
        .name(TEST_PLANNING_THREAD.to_owned())
        .spawn(move || {
            let _running = RunningPlanning(threads);
            let planned: Planned = Python::attach(|py| {
                let planned: Planned = plan(py, &request);
                drop(request);
                planned
            });
            let _receiver_gone = sender.send(planned);
        });
    if let Err(error) = spawned {
        planning_threads(py)?.get().finished();
        return Err(PyRuntimeError::new_err(error.to_string()));
    }
    Py::new(py, SqlTestPlanning::of(Some(receiver), None))
}

/// Join SQL test planning threads at exit: finalization ending one inside native planning aborts.
#[pyfunction]
fn join_sql_test_planning_threads(py: Python<'_>) -> PyResult<()> {
    signal_interpreter_exit();
    let threads: &PlanningThreads = planning_threads(py)?.get();
    py.detach(|| threads.wait(PLANNING_EXIT_JOIN));
    Ok(())
}

/// Start SQL test planning on a native thread, or resolve to no tests when disabled.
#[pyfunction]
#[pyo3(signature = (**kwargs))]
fn start_sql_test_planning<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Py<SqlTestPlanning>> {
    let args = KeywordArguments::read(
        py,
        "start_sql_test_planning",
        kwargs,
        (
            &[
                "project",
                "adapter",
                "selected_keys",
                "sql_test_selection",
                "enabled",
            ],
            &[],
        ),
    )?;
    start_planning(
        py,
        PlanningRequest {
            project: args.value("project")?.unbind(),
            adapter: args.value("adapter")?.unbind(),
            selected_keys: args.value("selected_keys")?.unbind(),
            sql_test_selection: args.value("sql_test_selection")?.unbind(),
        },
        args.flag("enabled")?,
    )
}

/// Add the planning handle and entry points, and join planning threads at interpreter exit.
pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SqlTestPlanning>()?;
    module.add_function(wrap_pyfunction!(start_sql_test_planning, module)?)?;
    let hook = wrap_pyfunction!(join_sql_test_planning_threads, module)?;
    module.add_function(hook.clone())?;
    imported(module.py(), "atexit", "register")?.call1((hook,))?;
    Ok(())
}
