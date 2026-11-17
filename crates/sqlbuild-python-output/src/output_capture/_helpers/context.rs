//! The integration-installed command-output context of the active command.

use std::sync::Mutex;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyResult, Python, pyclass, pyfunction, pymethods,
};
use pyo3::sync::PyOnceLock;
use pyo3::types::PyTuple;
use sqlbuild_host::entries::models::ExecutorHost;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::context_variable::context_variable;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

const CONTEXT_VARIABLE: &str = "sqlbuild_output_capture_context";
static CURRENT_CONTEXT: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// Attaches opaque integration context to command-output records while entered.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct CommandOutputContext {
    external_context: Py<PyAny>,
    token: Mutex<Option<Py<PyAny>>>,
}

#[pymethods]
impl CommandOutputContext {
    fn __enter__(&self, py: Python<'_>) -> PyResult<()> {
        let context = call_with(
            &host_entry(py, ExecutorHost::OutputCaptureContext)?,
            &[(
                "external_context",
                imported(py, "builtins", "dict")?.call1((self.external_context.bind(py),))?,
            )],
        )?;
        let token = context_variable(py, &CURRENT_CONTEXT, CONTEXT_VARIABLE)?
            .call_method1("set", (context,))?;
        *self
            .token
            .lock()
            .map_err(|_| PyRuntimeError::new_err("command output context lock is poisoned"))? =
            Some(token.unbind());
        Ok(())
    }

    #[pyo3(signature = (*_exc_info))]
    fn __exit__(&self, py: Python<'_>, _exc_info: &Bound<'_, PyTuple>) -> PyResult<bool> {
        let token = self
            .token
            .lock()
            .map_err(|_| PyRuntimeError::new_err("command output context lock is poisoned"))?
            .take();
        if let Some(token) = token {
            context_variable(py, &CURRENT_CONTEXT, CONTEXT_VARIABLE)?
                .call_method1("reset", (token,))?;
        }
        Ok(false)
    }
}

/// Attach opaque integration context to command-output records.
#[pyfunction]
#[pyo3(signature = (*, external_context))]
pub(crate) fn command_output_context(external_context: Py<PyAny>) -> CommandOutputContext {
    CommandOutputContext {
        external_context,
        token: Mutex::new(None),
    }
}

/// The optional integration context of the active command.
pub(crate) fn current_context(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    context_variable(py, &CURRENT_CONTEXT, CONTEXT_VARIABLE)?.call_method0("get")
}

/// Return optional integration context for the active command.
#[pyfunction]
pub(crate) fn current_output_capture_context(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    current_context(py)
}
