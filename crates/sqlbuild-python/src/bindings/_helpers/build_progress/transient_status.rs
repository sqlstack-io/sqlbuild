//! One transient status line with a spinner on a terminal and plain lines elsewhere.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyclass, pymethods};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::progress::main::format_status_spinner_line::format_status_spinner_line;

use crate::bindings::_helpers::boundary::python_calls::{imported, within};
use crate::bindings::_helpers::build_progress::terminal::{
    CLEAR_LINE, HIDE_CURSOR, SHOW_CURSOR, SPINNER_FRAMES, SPINNER_TICK_SECONDS, claim, flush,
    join_spinner, release, shared_lines, start_spinner, terminal_columns, tty_flag, write,
};
use crate::bindings::_helpers::records::fields::field as native_field;

const NO_PROGRESS_VARIABLE: &str = "SQLBUILD_NO_PROGRESS";
const PROGRESS_DISABLED_VALUES: [&str; 4] = ["1", "on", "true", "yes"];
const PREFLIGHT_COMPLETION_PREFIX: &str = "Prepared ";

/// Mutable reporter state; only held briefly and never across Python calls.
#[derive(Default)]
struct StatusState {
    message: Option<String>,
    active: bool,
    drawn: bool,
    frame: usize,
    stop_event: Option<Py<PyAny>>,
    threads: Vec<Py<PyAny>>,
    exit_hook_registered: bool,
}

/// Render one transient status line with optional spinner updates.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct TransientStatusReporter {
    stream: Py<PyAny>,
    lines: Py<PyAny>,
    lock: Py<PyAny>,
    style: CliStyle,
    enabled: bool,
    state: Mutex<StatusState>,
}

impl TransientStatusReporter {
    fn state(&self) -> MutexGuard<'_, StatusState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn locked<T>(&self, py: Python<'_>, body: impl FnOnce() -> PyResult<T>) -> PyResult<T> {
        within(self.lock.bind(py), |_| body())
    }

    pub(crate) fn write_persistent(&self, py: Python<'_>, text: &str) -> PyResult<()> {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("stream", self.stream.bind(py))?;
        options.set_item("text", text)?;
        self.lines
            .bind(py)
            .call_method("write_persistent", (), Some(&options))?;
        Ok(())
    }

    fn write_message(&self, py: Python<'_>, message: &str, dim_output: bool) -> PyResult<()> {
        let formatted: String = if dim_output {
            self.style.muted(message)
        } else {
            message.to_owned()
        };
        self.write_persistent(py, &format!("{formatted}\n"))
    }

    fn draw(&self, py: Python<'_>) -> PyResult<()> {
        let (message, frame) = {
            let mut state = self.state();
            let Some(message) = state.message.clone().filter(|_| state.drawn) else {
                return Ok(());
            };
            let frame: usize = state.frame;
            state.frame = (frame + 1) % SPINNER_FRAMES.len();
            (message, frame)
        };
        let line: String = format_status_spinner_line(
            SPINNER_FRAMES[frame],
            &message,
            terminal_columns(py)?,
            self.style,
        );
        let stream: &Bound<'_, PyAny> = self.stream.bind(py);
        write(stream, &format!("{CLEAR_LINE}{line}"))?;
        flush(stream)
    }

    fn enter_status(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        let register_hook: bool =
            !std::mem::replace(&mut reporter.state().exit_hook_registered, true);
        if register_hook {
            imported(py, "atexit", "register")?.call1((native_field(slf, "close")?,))?;
        }
        let stream: &Bound<'_, PyAny> = reporter.stream.bind(py);
        write(stream, HIDE_CURSOR)?;
        reporter.state().drawn = true;
        reporter.draw(py)?;
        let (event, thread) = start_spinner(slf.as_any())?;
        let mut state = reporter.state();
        state.stop_event = Some(event.unbind());
        state.threads.push(thread.unbind());
        Ok(())
    }

    fn close_status(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        reporter.locked(py, || {
            release(reporter.lines.bind(py), slf.as_any())?;
            reporter.exit_status(py)?;
            let mut state = reporter.state();
            state.message = None;
            state.active = false;
            Ok(())
        })
    }

    fn exit_status(&self, py: Python<'_>) -> PyResult<()> {
        let (drawn, event) = {
            let mut state = self.state();
            (std::mem::take(&mut state.drawn), state.stop_event.take())
        };
        if let Some(event) = event {
            event.bind(py).call_method0("set")?;
        }
        if drawn {
            let stream: &Bound<'_, PyAny> = self.stream.bind(py);
            write(stream, &format!("{CLEAR_LINE}{SHOW_CURSOR}"))?;
            flush(stream)?;
        }
        Ok(())
    }
}

#[pymethods]
impl TransientStatusReporter {
    #[new]
    #[pyo3(signature = (*, stream, use_color = false, enabled = true))]
    pub(crate) fn new(
        py: Python<'_>,
        stream: &Bound<'_, PyAny>,
        use_color: bool,
        enabled: bool,
    ) -> PyResult<Self> {
        let lines: Bound<'_, PyAny> = shared_lines(py)?;
        Ok(Self {
            enabled: enabled && !progress_spinners_disabled() && tty_flag(stream)?,
            stream: stream.clone().unbind(),
            lock: native_field(&lines, "lock")?.unbind(),
            lines: lines.unbind(),
            style: CliStyle { use_color },
            state: Mutex::new(StatusState::default()),
        })
    }

    pub(crate) fn start(slf: &Bound<'_, Self>, message: String) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        if reporter.enabled {
            return reporter.locked(py, || {
                Self::close_status(slf)?;
                reporter.state().message = Some(message);
                Self::enter_status(slf)?;
                claim(
                    reporter.lines.bind(py),
                    reporter.stream.bind(py),
                    slf.as_any(),
                )?;
                reporter.state().active = true;
                Ok(())
            });
        }
        reporter.state().active = true;
        reporter.write_message(py, &message, true)
    }

    pub(crate) fn update(&self, py: Python<'_>, message: String) -> PyResult<()> {
        if self.enabled {
            let updated: bool = self.locked(py, || {
                let drawn: bool = {
                    let mut state = self.state();
                    if state.message.is_none() {
                        return Ok(false);
                    }
                    state.message = Some(message.clone());
                    state.drawn
                };
                if drawn {
                    self.draw(py)?;
                }
                Ok(true)
            })?;
            if updated {
                return Ok(());
            }
        }
        self.write_message(py, &message, true)
    }

    #[pyo3(signature = (*, message, blank_line_after = false))]
    pub(crate) fn complete(
        slf: &Bound<'_, Self>,
        message: &str,
        blank_line_after: bool,
    ) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        Self::close(slf)?;
        slf.get().write_message(py, message, true)?;
        if blank_line_after {
            slf.get().write_blank_line(py)?;
        }
        Ok(())
    }

    /// Complete with a pre-styled message written verbatim.
    #[pyo3(signature = (*, message, blank_line_after = false))]
    pub(crate) fn complete_styled(
        slf: &Bound<'_, Self>,
        message: &str,
        blank_line_after: bool,
    ) -> PyResult<()> {
        Self::close(slf)?;
        let blank: &str = if blank_line_after { "\n" } else { "" };
        slf.get()
            .write_persistent(slf.py(), &format!("{message}\n{blank}"))
    }

    pub(crate) fn error(slf: &Bound<'_, Self>, message: &str) -> PyResult<()> {
        Self::close(slf)?;
        slf.get().write_message(slf.py(), message, false)
    }

    fn close(slf: &Bound<'_, Self>) -> PyResult<()> {
        Self::close_status(slf)?;
        let threads: Vec<Py<PyAny>> = std::mem::take(&mut slf.get().state().threads);
        for thread in threads {
            join_spinner(thread.bind(slf.py()))?;
        }
        Ok(())
    }

    /// Erase the live status so a persistent line can be written in its place; the spinner idles.
    fn clear_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        if std::mem::take(&mut self.state().drawn) {
            let stream: &Bound<'_, PyAny> = self.stream.bind(py);
            write(stream, &format!("{CLEAR_LINE}{SHOW_CURSOR}"))?;
            flush(stream)?;
        }
        Ok(())
    }

    /// Draw the live status again below persistent output, reusing its running spinner.
    fn redraw_transient_line(slf: &Bound<'_, Self>) -> PyResult<()> {
        let reporter: &Self = slf.get();
        let (redraw, spinning) = {
            let state = reporter.state();
            (
                state.message.is_some() && !state.drawn,
                state.stop_event.is_some(),
            )
        };
        if !redraw {
            return Ok(());
        }
        if !spinning {
            return Self::enter_status(slf);
        }
        write(reporter.stream.bind(slf.py()), HIDE_CURSOR)?;
        reporter.state().drawn = true;
        reporter.draw(slf.py())
    }

    /// Render one preflight progress update.
    fn report_preflight_progress(slf: &Bound<'_, Self>, message: String) -> PyResult<()> {
        if message.starts_with(PREFLIGHT_COMPLETION_PREFIX) {
            return Self::complete(slf, &message, true);
        }
        if !slf.get().state().active {
            return Self::start(slf, message);
        }
        slf.get().update(slf.py(), message)
    }

    fn write_blank_line(&self, py: Python<'_>) -> PyResult<()> {
        self.write_persistent(py, "\n")
    }

    fn _spin_until_stopped(&self, py: Python<'_>, stop_event: &Bound<'_, PyAny>) -> PyResult<()> {
        while !stop_event
            .call_method1("wait", (SPINNER_TICK_SECONDS,))?
            .is_truthy()?
        {
            self.locked(py, || {
                if stop_event.call_method0("is_set")?.is_truthy()? {
                    return Ok(());
                }
                self.draw(py)
            })?;
        }
        Ok(())
    }
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
