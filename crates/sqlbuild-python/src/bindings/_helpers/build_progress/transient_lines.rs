//! The coordinator of one live transient terminal line and persistent writes.

use std::sync::{Mutex, MutexGuard, PoisonError};

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::build_progress::terminal::tty_flag;

/// The stream a transient line is drawn on and the renderer that owns it.
type ActiveLine = (Py<PyAny>, Py<PyAny>);

static SHARED_COORDINATOR: PyOnceLock<Py<TransientLineCoordinator>> = PyOnceLock::new();

/// Keep persistent terminal lines from landing on a live spinner or status line.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct TransientLineCoordinator {
    /// The re-entrant lock every writer holds while it draws.
    #[pyo3(get)]
    pub(crate) lock: Py<PyAny>,
    active: Mutex<Option<ActiveLine>>,
}

impl TransientLineCoordinator {
    fn active(&self) -> MutexGuard<'_, Option<ActiveLine>> {
        self.active.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Run `body` while holding the coordinator lock.
    fn locked<T>(&self, py: Python<'_>, body: impl FnOnce() -> PyResult<T>) -> PyResult<T> {
        let lock: &Bound<'_, PyAny> = self.lock.bind(py);
        lock.call_method0("acquire")?;
        let result: PyResult<T> = body();
        lock.call_method0("release")?;
        result
    }

    /// The owner whose line a write to `stream` would land on.
    fn interrupted_owner<'py>(
        &self,
        stream: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py: Python<'py> = stream.py();
        let Some((owner_stream, owner)) = self
            .active()
            .as_ref()
            .map(|(owner_stream, owner)| (owner_stream.bind(py).clone(), owner.bind(py).clone()))
        else {
            return Ok(None);
        };
        if owner_stream.is(stream) || (tty_flag(&owner_stream)? && tty_flag(stream)?) {
            return Ok(Some(owner));
        }
        Ok(None)
    }
}

#[pymethods]
impl TransientLineCoordinator {
    #[new]
    fn new(py: Python<'_>) -> PyResult<Self> {
        Ok(Self {
            lock: imported(py, "threading", "RLock")?.call0()?.unbind(),
            active: Mutex::new(None),
        })
    }

    /// Record the owner of the transient line currently drawn on a terminal stream.
    #[pyo3(signature = (*, stream, owner))]
    fn claim(&self, stream: &Bound<'_, PyAny>, owner: &Bound<'_, PyAny>) -> PyResult<()> {
        self.locked(stream.py(), || {
            *self.active() = Some((stream.clone().unbind(), owner.clone().unbind()));
            Ok(())
        })
    }

    /// Forget a transient owner once its line is no longer drawn.
    #[pyo3(signature = (*, owner))]
    fn release(&self, owner: &Bound<'_, PyAny>) -> PyResult<()> {
        self.locked(owner.py(), || {
            let mut active = self.active();
            if active
                .as_ref()
                .is_some_and(|(_, current)| current.is(owner))
            {
                *active = None;
            }
            Ok(())
        })
    }

    /// Erase and forget any live transient line, so nothing redraws it afterwards.
    fn clear_active(&self, py: Python<'_>) -> PyResult<()> {
        self.locked(py, || {
            let taken: Option<ActiveLine> = self.active().take();
            if let Some((_, owner)) = taken {
                owner.bind(py).call_method0("clear_transient_line")?;
            }
            Ok(())
        })
    }

    /// Write lasting text, clearing and then redrawing any live transient line.
    #[pyo3(signature = (*, stream, text))]
    pub(crate) fn write_persistent(
        &self,
        stream: &Bound<'_, PyAny>,
        text: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.locked(stream.py(), || {
            let owner: Option<Bound<'_, PyAny>> = self.interrupted_owner(stream)?;
            if let Some(owner) = &owner {
                owner.call_method0("clear_transient_line")?;
            }
            stream.call_method1("write", (text,))?;
            stream.call_method0("flush")?;
            if let Some(owner) = &owner {
                owner.call_method0("redraw_transient_line")?;
            }
            Ok(())
        })
    }
}

/// The coordinator shared by every terminal writer in this process.
pub(crate) fn shared_coordinator(py: Python<'_>) -> PyResult<Bound<'_, TransientLineCoordinator>> {
    SHARED_COORDINATOR
        .get_or_try_init(py, || Py::new(py, TransientLineCoordinator::new(py)?))
        .map(|coordinator| coordinator.bind(py).clone())
}

/// Return the coordinator shared by every terminal writer in this process.
#[pyfunction]
pub(crate) fn shared_transient_line_coordinator(
    py: Python<'_>,
) -> PyResult<Bound<'_, TransientLineCoordinator>> {
    shared_coordinator(py)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<TransientLineCoordinator>()?;
    module.add_function(wrap_pyfunction!(shared_transient_line_coordinator, module)?)
}
