//! One transient status line with a spinner on a terminal and plain lines elsewhere.

use crate::build_progress::models::TransientStatusReporter;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};

use crate::build_progress::_helpers::terminal::tty_flag;

const NO_PROGRESS_VARIABLE: &str = "SQLBUILD_NO_PROGRESS";
const PROGRESS_DISABLED_VALUES: [&str; 4] = ["1", "on", "true", "yes"];
pub(crate) const PREFLIGHT_COMPLETION_PREFIX: &str = "Prepared ";

/// Mutable reporter state; only held briefly and never across Python calls.
#[derive(Debug, Default)]
pub(crate) struct StatusState {
    pub(crate) message: Option<String>,
    pub(crate) active: bool,
    pub(crate) drawn: bool,
    pub(crate) frame: usize,
    pub(crate) stop_event: Option<Py<PyAny>>,
    pub(crate) threads: Vec<Py<PyAny>>,
    pub(crate) exit_hook_registered: bool,
}

/// Whether `SQLBUILD_NO_PROGRESS` turns transient spinners off.
pub(crate) fn progress_spinners_disabled() -> bool {
    std::env::var(NO_PROGRESS_VARIABLE)
        .is_ok_and(|value| PROGRESS_DISABLED_VALUES.contains(&value.trim().to_lowercase().as_str()))
}

/// A spinner reporter on stderr, or `None` when stderr is not a terminal or progress is off.
fn stderr_reporter(py: Python<'_>) -> PyResult<Option<Py<TransientStatusReporter>>> {
    let stderr: Bound<'_, PyAny> = py.import("sys")?.getattr("stderr")?;
    if progress_spinners_disabled() || !tty_flag(&stderr)? {
        return Ok(None);
    }
    Py::new(py, TransientStatusReporter::new(py, &stderr, false, true)?).map(Some)
}

/// Run `body` under a stderr spinner when `enabled` and stderr is an interactive terminal.
pub(crate) fn maybe_status<'py, T>(
    py: Python<'py>,
    (message, enabled): (&str, bool),
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    let Some(reporter) = enabled.then(|| stderr_reporter(py)).transpose()?.flatten() else {
        return body();
    };
    let reporter: Bound<'py, TransientStatusReporter> = reporter.into_bound(py);
    TransientStatusReporter::start(&reporter, message.to_owned())?;
    let result: PyResult<T> = body();
    TransientStatusReporter::close(&reporter)?;
    result
}

/// Run `body`, drawing a stderr spinner named `message` only once it has run for `delay`.
pub(crate) fn delayed_status<'py, T>(
    py: Python<'py>,
    (message, delay): (&str, Duration),
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    let Some(reporter) = stderr_reporter(py)? else {
        return body();
    };
    let (finished, waiting) = mpsc::channel::<()>();
    let pending: Py<TransientStatusReporter> = reporter.clone_ref(py);
    let message: String = message.to_owned();
    let timer = std::thread::spawn(move || -> PyResult<()> {
        if waiting.recv_timeout(delay) != Err(RecvTimeoutError::Timeout) {
            return Ok(());
        }
        Python::attach(|py| TransientStatusReporter::start(pending.bind(py), message))
    });
    let result: PyResult<T> = body();
    let _ = finished.send(());
    let started: PyResult<()> = py.detach(|| timer.join()).unwrap_or_else(|_| Ok(()));
    TransientStatusReporter::close(reporter.bind(py))?;
    started?;
    result
}

/// A native status reporter writing to `stream`.
pub(crate) fn status_reporter<'py>(
    stream: &Bound<'py, PyAny>,
    use_color: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let reporter: TransientStatusReporter =
        TransientStatusReporter::new(stream.py(), stream, use_color, true)?;
    Ok(Bound::new(stream.py(), reporter)?.into_any())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<TransientStatusReporter>()
}
