//! One watched command: the watch started before it and settled after it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use pyo3::exceptions::{PyKeyboardInterrupt, PyRuntimeError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyBytes;

use crate::boundary::main::imported::imported;
use crate::native_host::_helpers::forced_exit::{announce, force_exit};
use crate::native_host::_helpers::interrupt::WATCHED_INTERRUPT;
use crate::native_host::_helpers::signal_watcher::watch_signals;
use crate::native_host::_helpers::terminal_settings::terminal_settings;
use crate::native_host::constants::{GRACE, INTERRUPTED_CODE, POLL, STOP_BYTE, TERMINATED_CODE};
use crate::native_host::models::{InterruptSession, InterruptWatch};

/// Start watching; `None` leaves an ignored or embedder's SIGINT handler alone.
pub(crate) fn start_watching(
    py: Python<'_>,
    terminated: fn() -> bool,
) -> PyResult<Option<InterruptSession>> {
    let signal = py.import("signal")?;
    let default: Bound<'_, PyAny> = signal.getattr("default_int_handler")?;
    let handler = signal.call_method1("getsignal", (signal.getattr("SIGINT")?,))?;
    if !handler.is(&default) {
        return Ok(None);
    }
    let sockets: Bound<'_, PyAny> = imported(py, "socket", "socketpair")?.call0()?;
    let (receiver, sender) = (sockets.get_item(0)?, sockets.get_item(1)?);
    sender.call_method1("setblocking", (false,))?;
    let watch: Arc<InterruptWatch> = Arc::new(InterruptWatch {
        receiver: receiver.unbind(),
        terminal: terminal_settings(py),
        terminated,
        pressed: AtomicBool::new(false),
        announced: AtomicBool::new(false),
    });
    let previous = signal.call_method1("set_wakeup_fd", (sender.call_method0("fileno")?,))?;
    WATCHED_INTERRUPT.store(false, Ordering::SeqCst);
    let watched: Arc<InterruptWatch> = Arc::clone(&watch);
    let watcher = std::thread::Builder::new()
        .name("sqlbuild-interrupt".to_owned())
        .spawn(move || Python::attach(|py| watch_signals(py, &watched)))?;
    Ok(Some(InterruptSession {
        watch,
        watcher,
        sender: sender.unbind(),
        previous_wakeup: previous.unbind(),
    }))
}

/// Stop watching and give the command's exit code, or 130 when Ctrl+C stopped it.
pub(crate) fn settle(
    py: Python<'_>,
    session: InterruptSession,
    ran: PyResult<i64>,
) -> PyResult<i64> {
    let signal = py.import("signal")?;
    let sigint: Bound<'_, PyAny> = signal.getattr("SIGINT")?;
    let ignore = || signal.call_method1("signal", (&sigint, signal.getattr("SIG_IGN")?));
    let late: bool = ignore().is_err();
    if late {
        drop(ignore());
    }
    signal.call_method1("set_wakeup_fd", (session.previous_wakeup.bind(py),))?;
    session
        .sender
        .bind(py)
        .call_method1("sendall", (PyBytes::new(py, &[STOP_BYTE]),))?;
    let watcher = session.watcher;
    match py.detach(|| watcher.join()) {
        Ok(watched) => watched?,
        Err(_panicked) => return Err(PyRuntimeError::new_err("the interrupt watcher panicked")),
    }
    let terminated: bool = ran.as_ref().is_ok_and(|code| *code == TERMINATED_CODE);
    let raised: bool = ran
        .as_ref()
        .is_err_and(|error| error.is_instance_of::<PyKeyboardInterrupt>(py));
    if terminated || !(raised || late || session.watch.pressed.load(Ordering::SeqCst)) {
        signal.call_method1("signal", (&sigint, signal.getattr("default_int_handler")?))?;
        return ran;
    }
    end_interrupted(py, &session.watch)
}

/// Say so, give threads left running the grace period, then give 130 or exit past them.
fn end_interrupted(py: Python<'_>, watch: &InterruptWatch) -> PyResult<i64> {
    announce(watch);
    let deadline: Instant = Instant::now() + GRACE;
    while threads_left_running(py)? {
        if Instant::now() >= deadline {
            force_exit(py, watch)?;
        }
        py.detach(|| std::thread::sleep(POLL));
    }
    Ok(INTERRUPTED_CODE)
}

/// Whether a thread interpreter exit would wait for still has a frame; `is_alive` can't say, as an interrupted `join` marks a running thread stopped.
fn threads_left_running(py: Python<'_>) -> PyResult<bool> {
    let running: Bound<'_, PyAny> = imported(py, "sys", "_current_frames")?.call0()?;
    let threading = py.import("threading")?;
    let current: Bound<'_, PyAny> = threading.call_method0("current_thread")?;
    for thread in threading.call_method0("enumerate")?.try_iter()? {
        let thread: Bound<'_, PyAny> = thread?;
        if !thread.is(&current)
            && !thread.getattr("daemon")?.is_truthy()?
            && running.contains(thread.getattr("ident")?)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}
