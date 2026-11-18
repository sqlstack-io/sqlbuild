//! Resolve host entries: targets read once from `sqlbuild._native_host`, attributes read live.

use crate::boundary::main::within::within;
use crate::build_progress::main::progress::write_persistent::write_persistent;
use pyo3::PyErr;
use pyo3::exceptions::PyImportError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple, PyTupleMethods};
use sqlbuild_host::entries::constants::{HOST_ENTRIES, HOST_MODULE, HOST_TARGETS_FUNCTION};
use sqlbuild_host::entries::models::{HostId, UserCodeHost};
use sqlbuild_host::entries::types::HostEntries;
use std::path::PathBuf;

/// One entry's target module path and attribute name.
type Target = (Py<PyString>, Py<PyString>);

/// Every entry's target, grouped and ordered as `HOST_ENTRIES`.
static TARGETS: PyOnceLock<Vec<Vec<Target>>> = PyOnceLock::new();

fn missing(name: &str) -> PyErr {
    PyImportError::new_err(format!("{HOST_MODULE} does not define host entry '{name}'"))
}

/// Read every entry's target from the host module, failing on the first entry it lacks.
fn host_targets(py: Python<'_>) -> PyResult<Vec<Vec<Target>>> {
    let declared: Bound<'_, PyDict> = py
        .import(HOST_MODULE)?
        .getattr(HOST_TARGETS_FUNCTION)?
        .call0()?
        .cast_into::<PyDict>()?;
    let mut groups: Vec<Vec<Target>> = Vec::with_capacity(HOST_ENTRIES.len());
    for group in HOST_ENTRIES {
        let mut targets: Vec<Target> = Vec::with_capacity(group.len());
        for entry in group {
            let target: Bound<'_, PyTuple> = declared
                .get_item(entry.name)?
                .ok_or_else(|| missing(entry.name))?
                .cast_into::<PyTuple>()?;
            targets.push((
                target.get_item(0)?.cast_into::<PyString>()?.unbind(),
                target.get_item(1)?.cast_into::<PyString>()?.unbind(),
            ));
        }
        groups.push(targets);
    }
    Ok(groups)
}

/// The host entries of one interpreter, read through `sqlbuild._native_host`.
#[derive(Clone, Copy)]
pub(crate) struct PythonHost<'py>(pub(crate) Python<'py>);

impl<'py> HostEntries for PythonHost<'py> {
    type Entry = Bound<'py, PyAny>;
    type Error = PyErr;

    fn entry(&self, id: HostId) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = self.0;
        let targets: &Vec<Vec<Target>> = TARGETS.get_or_try_init(py, || host_targets(py))?;
        let (module, attribute): &Target = &targets[id.edge as usize][id.index];
        py.import(module.bind(py))?.getattr(attribute.bind(py))
    }
}

/// The host entry `id`: its target module imported and the attribute read on every call.
pub(crate) fn entry<'py>(py: Python<'py>, id: impl Into<HostId>) -> PyResult<Bound<'py, PyAny>> {
    PythonHost(py).entry(id.into())
}

/// Write through the live console, clearing then redrawing any spinner line; errors surface.
pub(crate) fn write_console(py: Python<'_>, stream: &str, text: &str) -> PyResult<()> {
    let stream = py.import("sys")?.getattr(stream)?;
    write_persistent(py, &stream, text)
}

/// Flush the interpreter's live console `stream` so text written before a child process appears first.
pub(crate) fn flush_console(py: Python<'_>, stream: &str) -> PyResult<()> {
    py.import("sys")?.getattr(stream)?.call_method0("flush")?;
    Ok(())
}

/// The interpreter running SQLBuild, which child processes reuse.
pub(crate) fn python_executable(py: Python<'_>) -> PyResult<PathBuf> {
    py.import("sys")?.getattr("executable")?.extract()
}

/// Whether this interpreter can import the top-level module `name`.
pub(crate) fn module_installed(py: Python<'_>, name: &str) -> PyResult<bool> {
    Ok(!py
        .import("importlib.util")?
        .call_method1("find_spec", (name,))?
        .is_none())
}

/// Keep user prints off machine-readable stdout while running project code.
pub(crate) fn redirect_stdout_to_stderr(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    py.import("contextlib")?
        .getattr("redirect_stdout")?
        .call1((py.import("sys")?.getattr("stderr")?,))
}

/// `log_debug_event(logger=logging.getLogger(logger), message=message)`.
pub(crate) fn log_debug_message(py: Python<'_>, logger: &str, message: &str) -> PyResult<()> {
    let kwargs: Bound<'_, PyDict> = PyDict::new(py);
    kwargs.set_item(
        "logger",
        py.import("logging")?
            .getattr("getLogger")?
            .call1((logger,))?,
    )?;
    kwargs.set_item("message", message)?;
    entry(
        py,
        sqlbuild_host::entries::models::ExecutorHost::LogDebugEvent,
    )?
    .call((), Some(&kwargs))
    .map(|_| ())
}

/// `logging.getLogger(logger).warning(message)`.
pub(crate) fn log_warning_message(py: Python<'_>, logger: &str, message: &str) -> PyResult<()> {
    py.import("logging")?
        .getattr("getLogger")?
        .call1((logger,))?
        .call_method1("warning", (message,))
        .map(|_| ())
}

/// Run `body` inside one Python-node identity pass, so the identities it builds share source work.
pub(crate) fn within_identity_pass<'py, T>(
    py: Python<'py>,
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    let identity_pass: Bound<'py, PyAny> = entry(py, UserCodeHost::PythonIdentityPass)?.call0()?;
    within(&identity_pass, |_| body())
}
