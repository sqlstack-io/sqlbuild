//! Terminal width, the shared transient line coordinator and spinner threads of progress reporters.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};

use crate::boundary::main::call_with::call_with;
use crate::boundary::main::imported::imported;
use crate::build_progress::_helpers::progress::current_native_progress_projector;
use crate::build_progress::_helpers::transient_lines::shared_coordinator;
use crate::records::main::fields::field::field as native_field;

const DEFAULT_TERMINAL_SIZE: (i64, i64) = (120, 24);
const UNKNOWN_TERMINAL_SIZE: (i64, i64) = (0, 0);
const SPINNER_JOIN_SECONDS: f64 = 0.2;
const EXIT_JOIN_SECONDS: f64 = 1.0;
const SPINNER_THREAD_NAME: &str = "sqlbuild-progress-spinner";
const STOP_EVENT_ATTRIBUTE: &str = "_sqlbuild_stop_event";

/// The current terminal width, `shutil.get_terminal_size(fallback=(120, 24)).columns`.
pub(crate) fn terminal_columns(py: Python<'_>) -> PyResult<usize> {
    Ok(usize::try_from(reported_columns(py, DEFAULT_TERMINAL_SIZE)?).unwrap_or(0))
}

/// The terminal's width as it reports it, or `COLUMNS`; `None` when neither says.
pub(crate) fn known_terminal_columns(py: Python<'_>) -> PyResult<Option<usize>> {
    Ok(
        match usize::try_from(reported_columns(py, UNKNOWN_TERMINAL_SIZE)?) {
            Ok(columns) if columns > 0 => Some(columns),
            Ok(_) | Err(_) => None,
        },
    )
}

/// `shutil.get_terminal_size(fallback=fallback).columns`.
fn reported_columns(py: Python<'_>, fallback: (i64, i64)) -> PyResult<i64> {
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("fallback", fallback)?;
    imported(py, "shutil", "get_terminal_size")?
        .call((), Some(&options))?
        .getattr("columns")?
        .extract()
}

/// The process-wide coordinator of the one live transient line.
pub(crate) fn shared_lines(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    Ok(shared_coordinator(py)?.into_any())
}

/// Whether `stream` reports a terminal.
pub(crate) fn tty_flag(stream: &Bound<'_, PyAny>) -> PyResult<bool> {
    if !stream.hasattr("isatty")? {
        return Ok(false);
    }
    stream.call_method0("isatty")?.is_truthy()
}

/// The current native progress projector, if a projection is active.
pub(crate) fn projector(py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
    let projector: Bound<'_, PyAny> = current_native_progress_projector(py)?;
    Ok((!projector.is_none()).then(|| projector.unbind()))
}

/// The projector's canonical duration of a finished resource, `None` while it is pending.
pub(crate) fn consume_terminal(
    projector: &Bound<'_, PyAny>,
    resource_name: &str,
    resource_id: Option<&str>,
) -> PyResult<Option<f64>> {
    let options: Bound<'_, PyDict> = PyDict::new(projector.py());
    options.set_item("resource_name", resource_name)?;
    options.set_item("resource_id", resource_id)?;
    projector
        .call_method("consume_resource_terminal", (), Some(&options))?
        .extract()
}

/// Claim or release the live line for `owner`.
pub(crate) fn claim(
    lines: &Bound<'_, PyAny>,
    stream: &Bound<'_, PyAny>,
    owner: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let options: Bound<'_, PyDict> = PyDict::new(lines.py());
    options.set_item("stream", stream)?;
    options.set_item("owner", owner)?;
    lines.call_method("claim", (), Some(&options))?;
    Ok(())
}

pub(crate) fn release(lines: &Bound<'_, PyAny>, owner: &Bound<'_, PyAny>) -> PyResult<()> {
    let options: Bound<'_, PyDict> = PyDict::new(lines.py());
    options.set_item("owner", owner)?;
    lines.call_method("release", (), Some(&options))?;
    Ok(())
}

pub(crate) fn write(stream: &Bound<'_, PyAny>, text: &str) -> PyResult<()> {
    stream.call_method1("write", (text,))?;
    Ok(())
}

pub(crate) fn flush(stream: &Bound<'_, PyAny>) -> PyResult<()> {
    stream.call_method0("flush")?;
    Ok(())
}

/// Stop and join live spinners at exit: finalization ending one inside its native wait aborts.
#[pyfunction]
fn stop_progress_spinners(py: Python<'_>) -> PyResult<()> {
    let mut spinners: Vec<Bound<'_, PyAny>> = Vec::new();
    for thread in imported(py, "threading", "enumerate")?
        .call0()?
        .try_iter()?
    {
        let thread: Bound<'_, PyAny> = thread?;
        if native_field(&thread, "name")?.eq(SPINNER_THREAD_NAME)?
            && thread.hasattr(STOP_EVENT_ATTRIBUTE)?
        {
            native_field(&thread, STOP_EVENT_ATTRIBUTE)?.call_method0("set")?;
            spinners.push(thread);
        }
    }
    for thread in spinners {
        if thread.call_method0("is_alive")?.is_truthy()? {
            let options: Bound<'_, PyDict> = PyDict::new(py);
            options.set_item("timeout", EXIT_JOIN_SECONDS)?;
            thread.call_method("join", (), Some(&options))?;
        }
    }
    Ok(())
}

/// Start a daemon thread running `owner._spin_until_stopped(event)`; return the event and thread.
pub(crate) fn start_spinner<'py>(
    owner: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = owner.py();
    let event: Bound<'py, PyAny> = imported(py, "threading", "Event")?.call0()?;
    let thread: Bound<'py, PyAny> = call_with(
        &imported(py, "threading", "Thread")?,
        &[
            ("target", native_field(owner, "_spin_until_stopped")?),
            ("args", PyTuple::new(py, [&event])?.into_any()),
            ("daemon", PyBool::new(py, true).to_owned().into_any()),
            ("name", SPINNER_THREAD_NAME.into_bound_py_any(py)?),
        ],
    )?;
    thread.setattr(STOP_EVENT_ATTRIBUTE, &event)?;
    thread.call_method0("start")?;
    Ok((event, thread))
}

/// Join a stopped spinner thread briefly if it is still alive.
pub(crate) fn join_spinner(thread: &Bound<'_, PyAny>) -> PyResult<()> {
    if thread.call_method0("is_alive")?.is_truthy()? {
        let options: Bound<'_, PyDict> = PyDict::new(thread.py());
        options.set_item("timeout", SPINNER_JOIN_SECONDS)?;
        thread.call_method("join", (), Some(&options))?;
    }
    Ok(())
}

/// Add `stop_progress_spinners` and run it at exit, before finalization ends daemon threads.
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let hook = wrap_pyfunction!(stop_progress_spinners, module)?;
    module.add_function(hook.clone())?;
    imported(module.py(), "atexit", "register")?.call1((hook,))?;
    Ok(())
}
