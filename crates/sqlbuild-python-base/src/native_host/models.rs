//! Public records of native host.

use crate::native_host::_helpers::interrupt::INTERRUPTED;
use crate::native_host::_helpers::interrupt::SIGINT;
use pyo3::exceptions::PyKeyboardInterrupt;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyCFunction;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyTuple;
use sqlbuild_model_config::header_metadata::models::DynamicFamily;
use crate::native_host::_helpers::dialect::syntax;
use crate::native_host::types::Syntax;
use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::pyclass;
use pyo3::prelude::pymethods;

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
            let restored: PyResult<()> = (|| {
                let signal = py.import("signal")?;
                if signal
                    .call_method1("getsignal", (SIGINT,))?
                    .is(self.installed.bind(py))
                {
                    signal.call_method1("signal", (SIGINT, self.previous.bind(py)))?;
                }
                Ok(())
            })();
            drop(restored);
        });
        INTERRUPTED.set(false);
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
    pub fn attribute<'py>(
        &self,
        py: Python<'py>,
        name: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
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
