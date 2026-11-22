//! The thread that follows SIGINT while a command runs, leaving Python's handlers as they are.

use std::sync::atomic::Ordering;
use std::time::Instant;

use pyo3::exceptions::PyTimeoutError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::boundary::main::imported::imported;
use crate::native_host::_helpers::adapter_connects::connects_in_progress;
use crate::native_host::_helpers::forced_exit::{cancel_statements, force_exit};
use crate::native_host::_helpers::interrupt::WATCHED_INTERRUPT;
use crate::native_host::constants::{
    GRACE, INTERRUPT_BYTE, POLL, RECEIVE_BYTES, STEP_THREAD_PREFIX, STOP_BYTE,
};
use crate::native_host::main::host_entry::host_entry;
use crate::native_host::models::{InterruptPresses, InterruptWatch};

/// Whether a statement has ever run, and whether one runs now.
fn statement_activity(py: Python<'_>) -> PyResult<(bool, bool)> {
    host_entry(py, ExecutorHost::StatementActivity)?
        .call0()?
        .extract()
}

/// Read the wakeup descriptor until told to stop, forcing the exit where `may_force` allows.
pub(crate) fn watch_signals(py: Python<'_>, watch: &InterruptWatch) -> PyResult<()> {
    let receiver: &Bound<'_, PyAny> = watch.receiver.bind(py);
    let mut presses: Option<InterruptPresses> = None;
    loop {
        let wait: Option<f64> = presses.as_ref().map(|_| POLL.as_secs_f64());
        receiver.call_method1("settimeout", (wait,))?;
        let received: Vec<u8> = match receiver.call_method1("recv", (RECEIVE_BYTES,)) {
            Ok(received) => received.extract()?,
            Err(error) if error.is_instance_of::<PyTimeoutError>(py) => Vec::new(),
            Err(error) => return Err(error),
        };
        let count: usize = received
            .iter()
            .filter(|byte| **byte == INTERRUPT_BYTE)
            .count();
        let terminating: bool = (watch.terminated)();
        if count > 0 && !terminating {
            WATCHED_INTERRUPT.store(true, Ordering::SeqCst);
            presses = Some(pressed(py, presses, count));
            watch.pressed.store(true, Ordering::SeqCst);
        }
        if received.contains(&STOP_BYTE) {
            return Ok(());
        }
        if let Some(presses) = &presses
            && !terminating
            && presses.last.elapsed() >= GRACE
            && may_force(py, presses)
        {
            force_exit(py, watch)?;
        }
    }
}

/// Count presses; the first notes discovered steps and the connects in progress, and cancels the statements running then, as SIGTERM does, so a driver waiting in native code returns.
fn pressed(py: Python<'_>, presses: Option<InterruptPresses>, count: usize) -> InterruptPresses {
    let last: Instant = Instant::now();
    if let Some(presses) = presses {
        return InterruptPresses {
            count: presses.count + count,
            last,
            connecting: presses.connecting,
            stepping: presses.stepping,
        };
    }
    let stepping: bool = steps_have_threads(py).unwrap_or_default();
    let connecting = connects_in_progress(py).unwrap_or_default();
    if statement_activity(py).is_ok_and(|(_, running)| running) {
        drop(cancel_statements(py));
    }
    InterruptPresses {
        count,
        last,
        connecting,
        stepping,
    }
}

/// Whether ending the process cuts no cleanup short, or was asked for twice: the connect waiting at the press still waits, or no statement has ever run. One press never forces it once discovered steps had threads: their pool reports them within its own grace, and the command then records and exits.
fn may_force(py: Python<'_>, presses: &InterruptPresses) -> bool {
    if presses.count > 1 {
        return true;
    }
    if presses.stepping {
        return false;
    }
    if !presses.connecting.is_empty() {
        let current = connects_in_progress(py).unwrap_or_default();
        for frame in &presses.connecting {
            if is_running(frame, &current) {
                return true;
            }
        }
    }
    statement_activity(py).is_ok_and(|(ever, _)| !ever)
}

fn is_running(frame: &Py<PyAny>, running: &[Py<PyAny>]) -> bool {
    running.iter().any(|current| current.is(frame))
}

/// Whether a pool of discovered steps has threads, found by their name; it keeps them until it has settled its steps.
fn steps_have_threads(py: Python<'_>) -> PyResult<bool> {
    for thread in imported(py, "threading", "enumerate")?
        .call0()?
        .try_iter()?
    {
        let name: String = thread?.getattr("name")?.extract()?;
        if name.starts_with(STEP_THREAD_PREFIX) {
            return Ok(true);
        }
    }
    Ok(false)
}
