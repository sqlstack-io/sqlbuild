//! Public records of native host.

use crate::native_host::_helpers::dialect::syntax;
use crate::native_host::_helpers::interrupt::INTERRUPTED;
use crate::native_host::_helpers::interrupt::SIGINT;
use crate::native_host::_helpers::interrupt::restore_signal_handler;
use crate::native_host::types::Syntax;
use pyo3::exceptions::PyAttributeError;
use pyo3::exceptions::PyKeyboardInterrupt;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::prelude::pyclass;
use pyo3::prelude::pymethods;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyCFunction;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyTuple;
use sqlbuild_model_config::header_metadata::models::DynamicFamily;

/// Factory results, source locations and the cached public audit tuple are Python-owned edges.
pub struct AuditProjection {
    pub custom: Option<Vec<Py<PyAny>>>,
    pub generated: Vec<Py<PyAny>>,
    pub location: Py<PyAny>,
    pub records: PyOnceLock<Py<PyTuple>>,
}

/// A supplied constructor and its public family records; native callers retain the family specs.
pub struct FamilyProjection {
    pub(crate) class: Py<PyAny>,
    pub custom: bool,
    pub(crate) records: PyOnceLock<Py<PyTuple>>,
}

impl FamilyProjection {
    pub fn new(class: &Bound<'_, PyAny>, custom: bool) -> Self {
        Self {
            class: class.clone().unbind(),
            custom,
            records: PyOnceLock::new(),
        }
    }

    pub fn project<'py>(
        &self,
        py: Python<'py>,
        specs: &[DynamicFamily],
    ) -> PyResult<Bound<'py, PyTuple>> {
        self.records
            .get_or_try_init(py, || {
                let mut records: Vec<Bound<'_, PyAny>> = Vec::with_capacity(specs.len());
                for family in specs {
                    let fields = PyDict::new(py);
                    fields.set_item("name", &family.name)?;
                    fields.set_item("pivot_column", &family.pivot_column)?;
                    fields.set_item("value_column", &family.value_column)?;
                    fields.set_item("aggregate", &family.aggregate)?;
                    fields.set_item("type", &family.data_type)?;
                    fields.set_item("name_pattern", &family.name_pattern)?;
                    records.push(self.class.bind(py).call((), Some(&fields))?);
                }
                PyTuple::new(py, records).map(Bound::unbind)
            })
            .map(|value| value.bind(py).clone())
    }
}

/// The previous Python handler and the command's replacement, restored on every exit path.
#[derive(Debug)]
pub struct InterruptHandler {
    pub(crate) previous: Py<PyAny>,
    pub(crate) installed: Py<PyAny>,
}

impl InterruptHandler {
    /// Capture only Python's default main-thread handler; leave embedding applications' handlers alone.
    pub fn install(py: Python<'_>) -> PyResult<Option<Self>> {
        let threading = py.import("threading")?;
        if !threading
            .call_method0("current_thread")?
            .is(threading.call_method0("main_thread")?)
        {
            return Ok(None);
        }
        let signal = py.import("signal")?;
        let previous = signal.call_method1("getsignal", (SIGINT,))?;
        if !previous.is(signal.getattr("default_int_handler")?) {
            return Ok(None);
        }
        let installed = PyCFunction::new_closure(
            py,
            None,
            None,
            |_: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
                INTERRUPTED.set(true);
                Err(PyKeyboardInterrupt::new_err(()))
            },
        )?;
        INTERRUPTED.set(false);
        signal.call_method1("signal", (SIGINT, &installed))?;
        Ok(Some(Self {
            previous: previous.unbind(),
            installed: installed.into_any().unbind(),
        }))
    }
}

impl Drop for InterruptHandler {
    fn drop(&mut self) {
        Python::attach(|py| {
            drop(restore_signal_handler(
                py,
                SIGINT,
                (&self.installed, &self.previous),
            ));
        });
        INTERRUPTED.set(false);
    }
}

/// A command's handler on an otherwise unused signal that raises the command's error, so another thread can interrupt the main thread while SIGINT is ignored, without changing how SIGINT is handled.
#[derive(Debug)]
pub struct WakeHandler {
    pub signal: i64,
    pub(crate) previous: Py<PyAny>,
    pub(crate) installed: Py<PyAny>,
}

impl WakeHandler {
    /// The signal that interrupts the main thread: SIGINT where Python handles it, else the signal of a wake handler raising `error`, installed here; `None` when neither is available.
    pub fn interrupting_signal(
        py: Python<'_>,
        error: fn() -> PyErr,
    ) -> PyResult<(Option<i64>, Option<Self>)> {
        let signal = py.import("signal")?;
        if signal.call_method1("getsignal", (SIGINT,))?.is_callable() {
            return Ok((Some(SIGINT), None));
        }
        let wake: Option<Self> = Self::install(py, error)?;
        Ok((wake.as_ref().map(|wake| wake.signal), wake))
    }

    /// Install the handler on SIGRTMIN, or SIGUSR2 where there is none, while that signal takes its default action.
    fn install(py: Python<'_>, error: fn() -> PyErr) -> PyResult<Option<Self>> {
        let signal = py.import("signal")?;
        let name: &str = if signal.hasattr("SIGRTMIN")? {
            "SIGRTMIN"
        } else if signal.hasattr("SIGUSR2")? {
            "SIGUSR2"
        } else {
            return Ok(None);
        };
        let number: i64 = signal.getattr(name)?.extract()?;
        let previous = signal.call_method1("getsignal", (number,))?;
        if !previous.is(signal.getattr("SIG_DFL")?) {
            return Ok(None);
        }
        let installed = PyCFunction::new_closure(
            py,
            None,
            None,
            move |_: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
                Err(error())
            },
        )?;
        signal.call_method1("signal", (number, &installed))?;
        Ok(Some(Self {
            signal: number,
            previous: previous.unbind(),
            installed: installed.into_any().unbind(),
        }))
    }
}

impl Drop for WakeHandler {
    /// Restore the previous handler. A wake still pending raises on the first attempt and is consumed, so the second attempt restores.
    fn drop(&mut self) {
        Python::attach(|py| {
            let handlers = (&self.installed, &self.previous);
            drop(
                restore_signal_handler(py, self.signal, handlers)
                    .or_else(|_pending| restore_signal_handler(py, self.signal, handlers)),
            );
        });
    }
}

impl std::fmt::Debug for AuditProjection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuditProjection")
            .field("custom", &self.custom)
            .field("generated", &self.generated)
            .field("location", &self.location)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for FamilyProjection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FamilyProjection")
            .field("class", &self.class)
            .field("custom", &self.custom)
            .finish_non_exhaustive()
    }
}

/// The adapter's SQL syntax for a plan; anything that would reach the warehouse is refused.
#[pyclass(frozen, module = "sqlbuild._native", name = "Dialect")]
#[derive(Debug)]
pub struct Dialect {
    adapter: Py<PyAny>,
}

impl Dialect {
    /// The dialect of `adapter`.
    pub fn of<'py>(adapter: &Bound<'py, PyAny>) -> PyResult<Bound<'py, Dialect>> {
        Bound::new(
            adapter.py(),
            Dialect {
                adapter: adapter.clone().unbind(),
            },
        )
    }

    /// A dialect passed as a keyword argument; an adapter or anything else is refused.
    pub fn passed(value: Bound<'_, PyAny>) -> PyResult<Bound<'_, Dialect>> {
        Ok(value.cast_into::<Dialect>()?)
    }

    /// One syntax attribute of the adapter, such as `render_qualified_name`.
    pub fn attribute<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        if !syntax(name) {
            return Err(PyAttributeError::new_err(format!(
                "a plan's SQL dialect writes SQL only; `{name}` would reach the warehouse"
            )));
        }
        self.adapter.bind(py).getattr(name)
    }
}

impl<'py> Syntax<'py> for Bound<'py, Dialect> {
    fn syntax(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        self.get().attribute(self.py(), name)
    }
}

#[pymethods]
impl Dialect {
    #[new]
    #[pyo3(signature = (adapter))]
    fn new(adapter: Bound<'_, PyAny>) -> Self {
        Dialect {
            adapter: adapter.unbind(),
        }
    }

    /// Python code handed a dialect reads the same syntax attributes; anything else is refused.
    fn __getattr__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        self.attribute(py, name)
    }
}

/// One command's watch for Ctrl+C.
#[derive(Debug)]
pub(crate) struct InterruptWatch {
    /// The socket Python's wakeup descriptor feeds.
    pub(crate) receiver: Py<PyAny>,
    /// The terminal's descriptor and the settings it had when the command started.
    pub(crate) terminal: Option<(i32, Py<PyAny>)>,
    /// Whether SIGTERM is stopping the command, which then ends as a termination.
    pub(crate) terminated: fn() -> bool,
    pub(crate) pressed: std::sync::atomic::AtomicBool,
    pub(crate) announced: std::sync::atomic::AtomicBool,
}

/// The Ctrl+C presses so far.
#[derive(Debug)]
pub(crate) struct InterruptPresses {
    pub(crate) count: usize,
    pub(crate) last: std::time::Instant,
    /// The adapter connects in progress at the first press.
    pub(crate) connecting: Vec<Py<PyAny>>,
}

/// A watch while its command runs.
#[derive(Debug)]
pub(crate) struct InterruptSession {
    pub(crate) watch: std::sync::Arc<InterruptWatch>,
    pub(crate) watcher: std::thread::JoinHandle<PyResult<()>>,
    /// The wakeup descriptor's socket.
    pub(crate) sender: Py<PyAny>,
    pub(crate) previous_wakeup: Py<PyAny>,
}
