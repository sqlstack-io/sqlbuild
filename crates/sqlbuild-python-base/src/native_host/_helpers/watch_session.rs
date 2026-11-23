//! One watched command: the watch started before it and settled after it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use pyo3::exceptions::{PyKeyboardInterrupt, PyRuntimeError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};

use crate::boundary::main::imported::imported;
use crate::native_host::_helpers::forced_exit::{announce, force_exit};
use crate::native_host::_helpers::interrupt::{ACTIVE_WATCH, WATCHED_INTERRUPT};
use crate::native_host::_helpers::signal_watcher::{
    send_stop, send_sync, wakeup_descriptor, wakeup_pair, watch_signals,
};
use crate::native_host::_helpers::terminal_settings::terminal_settings;
use crate::native_host::constants::{GRACE, INTERRUPTED_CODE, POLL, SYNC_WAIT, TERMINATED_CODE};
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
    let (receiver, sender) = wakeup_pair()?;
    let watch: Arc<InterruptWatch> = Arc::new(InterruptWatch {
        receiver,
        sender,
        syncs: std::sync::Mutex::new(0),
        synced: std::sync::Condvar::new(),
        terminal: terminal_settings(py),
        terminated,
        pressed: AtomicBool::new(false),
        announced: AtomicBool::new(false),
    });
    let previous = signal.call_method1("set_wakeup_fd", (wakeup_descriptor(&watch.sender),))?;
    WATCHED_INTERRUPT.store(false, Ordering::SeqCst);
    *active_watch() = Some(Arc::clone(&watch));
    let watched: Arc<InterruptWatch> = Arc::clone(&watch);
    let watcher = std::thread::Builder::new()
        .name("sqlbuild-interrupt".to_owned())
        .spawn(move || Python::attach(|py| watch_signals(py, &watched)))?;
    Ok(Some(InterruptSession {
        watch,
        watcher,
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
    *active_watch() = None;
    send_stop(&session.watch.sender)?;
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

fn active_watch() -> std::sync::MutexGuard<'static, Option<Arc<InterruptWatch>>> {
    ACTIVE_WATCH
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Wait, without the interpreter, until the running command's watch has read every byte written before now, so a press already delivered counts however busy the machine is. A full socket means the watcher is behind already, and is not waited for.
pub(crate) fn catch_up(py: Python<'_>) -> PyResult<()> {
    let Some(watch) = active_watch().clone() else {
        return Ok(());
    };
    py.detach(|| {
        let syncs = watch
            .syncs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let before: u64 = *syncs;
        match send_sync(&watch.sender) {
            Ok(()) => {
                drop(
                    watch
                        .synced
                        .wait_timeout_while(syncs, SYNC_WAIT, |read| *read == before),
                );
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
            Err(error) => Err(error.into()),
        }
    })
}
