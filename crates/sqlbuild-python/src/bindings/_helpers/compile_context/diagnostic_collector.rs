//! Collect compile diagnostics so one compile reports every violation, not only the first.

use std::sync::Mutex;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{PyErr, pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_host::compile_context::types::CompileDiagnosticSink;

use sqlbuild_python_base::boundary::main::context_variable::context_variable;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

const CONTEXT_VARIABLE_NAME: &str = "sqlbuild_collected_compile_diagnostics";

static ACTIVE_COLLECTOR: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// The context variable holding the collection diagnostics are reported into, if any.
fn active_collector(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    context_variable(py, &ACTIVE_COLLECTOR, CONTEXT_VARIABLE_NAME)
}

/// Deduplicated compile diagnostics gathered while the collection is entered.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct CollectedCompileDiagnostics {
    by_key: Py<PyDict>,
    token: Mutex<Option<Py<PyAny>>>,
}

#[pymethods]
impl CollectedCompileDiagnostics {
    fn __enter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Self>> {
        let token: Bound<'_, PyAny> =
            active_collector(slf.py())?.call_method1("set", (slf.clone(),))?;
        *slf.get()
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(token.unbind());
        Ok(slf.clone())
    }

    #[pyo3(signature = (*_exception))]
    fn __exit__(&self, py: Python<'_>, _exception: &Bound<'_, PyTuple>) -> PyResult<bool> {
        let token: Option<Py<PyAny>> = self
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(token) = token {
            active_collector(py)?.call_method1("reset", (token,))?;
        }
        Ok(false)
    }

    /// Record one diagnostic; a repeated key keeps the first diagnostic.
    #[pyo3(signature = (*, key, diagnostic))]
    fn add(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        diagnostic: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.by_key
            .bind(py)
            .call_method1("setdefault", (key, diagnostic))?;
        Ok(())
    }

    /// The recorded diagnostics in deterministic key order.
    #[getter]
    pub(crate) fn diagnostics<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let by_key: &Bound<'py, PyDict> = self.by_key.bind(py);
        let keys: Bound<'py, PyAny> = imported(py, "builtins", "sorted")?.call1((by_key,))?;
        let diagnostics: Vec<Bound<'py, PyAny>> = keys
            .try_iter()?
            .map(|key| by_key.as_any().get_item(key?))
            .collect::<PyResult<_>>()?;
        PyTuple::new(py, diagnostics)
    }
}

/// A new collection; entering it collects diagnostics instead of raising the first one.
#[pyfunction]
pub(crate) fn collect_compile_diagnostics(
    py: Python<'_>,
) -> PyResult<Bound<'_, CollectedCompileDiagnostics>> {
    Bound::new(
        py,
        CollectedCompileDiagnostics {
            by_key: PyDict::new(py).unbind(),
            token: Mutex::new(None),
        },
    )
}

/// The compile diagnostic sink of one interpreter: the entered collection, else the first error.
#[derive(Clone, Copy)]
pub(crate) struct PythonCompileDiagnostics<'py>(pub(crate) Python<'py>);

impl<'py> CompileDiagnosticSink for PythonCompileDiagnostics<'py> {
    type Key = Bound<'py, PyAny>;
    type Diagnostic = Bound<'py, PyAny>;
    type Error = PyErr;

    fn report(&self, key: &Bound<'py, PyAny>, diagnostic: &Bound<'py, PyAny>) -> PyResult<()> {
        let py: Python<'py> = self.0;
        let collector: Bound<'_, PyAny> = active_collector(py)?.call_method0("get")?;
        if let Ok(collector) = collector.cast::<CollectedCompileDiagnostics>() {
            return collector.get().add(py, key, diagnostic);
        }
        let location: Bound<'_, PyAny> = native_field(diagnostic, "location")?;
        let path: Bound<'_, PyAny> = native_field(diagnostic, "path")?;
        let pointer: String = if !location.is_none() {
            format!(
                "\n  --> {}:{}",
                native_field(&location, "path")?
                    .call_method0("as_posix")?
                    .str()?,
                native_field(&location, "line")?.str()?
            )
        } else if !path.is_none() {
            format!("\n  --> {}", path.call_method0("as_posix")?.str()?)
        } else {
            String::new()
        };
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("code", native_field(diagnostic, "code")?)?;
        keywords.set_item("help", native_field(diagnostic, "help")?)?;
        let message: String = format!("{}{pointer}", native_field(diagnostic, "message")?.str()?);
        Err(PyErr::from_value(
            error_class(py, ErrorId::CompileInput)?.call((message,), Some(&keywords))?,
        ))
    }
}

/// Record a diagnostic in the active collection, or raise it when none is active.
#[pyfunction]
#[pyo3(signature = (*, key, diagnostic))]
pub(crate) fn report_compile_diagnostic(
    key: &Bound<'_, PyAny>,
    diagnostic: &Bound<'_, PyAny>,
) -> PyResult<()> {
    PythonCompileDiagnostics(key.py()).report(key, diagnostic)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CollectedCompileDiagnostics>()?;
    module.add_function(wrap_pyfunction!(collect_compile_diagnostics, module)?)?;
    module.add_function(wrap_pyfunction!(report_compile_diagnostic, module)?)
}
