//! The thread that follows SIGINT while a command runs, leaving Python's handlers as they are, and the socket pair it reads without the interpreter.

use std::io::{ErrorKind, Read, Write};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use pyo3::prelude::{Py, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::boundary::main::imported::imported;
use crate::native_host::_helpers::adapter_connects::connects_in_progress;
use crate::native_host::_helpers::forced_exit::{cancel_statements, force_exit};
use crate::native_host::_helpers::interrupt::WATCHED_INTERRUPT;
use crate::native_host::constants::{
    GRACE, INTERRUPT_BYTE, POLL, RECEIVE_BYTES, STEP_THREAD_PREFIX, STOP_BYTE, SYNC_BYTE,
};
use crate::native_host::main::host_entry::host_entry;
use crate::native_host::models::{InterruptPresses, InterruptWatch};
use crate::native_host::types::WakeupSocket;

/// Whether a statement has ever run, and whether one runs now.
fn statement_activity(py: Python<'_>) -> PyResult<(bool, bool)> {
    host_entry(py, ExecutorHost::StatementActivity)?
        .call0()?
        .extract()
}

/// Read the wakeup descriptor until told to stop, forcing the exit where `may_force` allows. A press is noted as soon as it is read, before the interpreter is taken back, so a command that returns meanwhile still ends as interrupted.
pub(crate) fn watch_signals(py: Python<'_>, watch: &InterruptWatch) -> PyResult<()> {
    let mut presses: Option<InterruptPresses> = None;
    loop {
        let wait: Option<Duration> = presses.as_ref().map(|_| POLL);
        let received: Vec<u8> = py.detach(|| {
            let received = receive(&watch.receiver, wait);
            if received
                .as_ref()
                .is_ok_and(|bytes| bytes.contains(&INTERRUPT_BYTE))
                && !(watch.terminated)()
            {
                WATCHED_INTERRUPT.store(true, Ordering::SeqCst);
            }
            if let Ok(bytes) = &received {
                acknowledge_syncs(watch, bytes);
            }
            received
        })?;
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

/// A connected pair: the end the watcher reads, and the non-blocking end Python's wakeup descriptor writes.
#[cfg(unix)]
pub(crate) fn wakeup_pair() -> std::io::Result<(WakeupSocket, WakeupSocket)> {
    let (receiver, sender) = std::os::unix::net::UnixStream::pair()?;
    sender.set_nonblocking(true)?;
    Ok((receiver, sender))
}

/// A connected pair over loopback, as Python's `socket.socketpair` makes one on Windows.
#[cfg(windows)]
pub(crate) fn wakeup_pair() -> std::io::Result<(WakeupSocket, WakeupSocket)> {
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let sender = std::net::TcpStream::connect(listener.local_addr()?)?;
    let (receiver, peer) = listener.accept()?;
    if peer != sender.local_addr()? {
        return Err(std::io::Error::other(
            "an unexpected connection to the wakeup socket",
        ));
    }
    sender.set_nonblocking(true)?;
    Ok((receiver, sender))
}

/// The descriptor `signal.set_wakeup_fd` takes for `sender`.
#[cfg(unix)]
pub(crate) fn wakeup_descriptor(sender: &WakeupSocket) -> i64 {
    i64::from(std::os::fd::AsRawFd::as_raw_fd(sender))
}

/// The socket handle `signal.set_wakeup_fd` takes for `sender`.
#[cfg(windows)]
pub(crate) fn wakeup_descriptor(sender: &WakeupSocket) -> i64 {
    i64::try_from(std::os::windows::io::AsRawSocket::as_raw_socket(sender)).unwrap_or(-1)
}

/// The bytes that arrived within `wait`, or until some arrive when `wait` is `None`; a closed sender reads as the stop byte.
pub(crate) fn receive(receiver: &WakeupSocket, wait: Option<Duration>) -> std::io::Result<Vec<u8>> {
    receiver.set_read_timeout(wait)?;
    let mut bytes: [u8; RECEIVE_BYTES] = [0; RECEIVE_BYTES];
    let mut reader: &WakeupSocket = receiver;
    match reader.read(&mut bytes) {
        Ok(0) => Ok(vec![STOP_BYTE]),
        Ok(read) => Ok(bytes[..read].to_vec()),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
            ) =>
        {
            Ok(Vec::new())
        }
        Err(error) => Err(error),
    }
}

/// Count the sync bytes just read and wake the command's end waiting on them.
fn acknowledge_syncs(watch: &InterruptWatch, bytes: &[u8]) {
    let read: u64 = bytes.iter().filter(|byte| **byte == SYNC_BYTE).count() as u64;
    if read > 0 {
        *watch
            .syncs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) += read;
        watch.synced.notify_all();
    }
}

/// Ask the watcher to confirm it has read everything written before this.
pub(crate) fn send_sync(sender: &WakeupSocket) -> std::io::Result<()> {
    let mut writer: &WakeupSocket = sender;
    writer.write_all(&[SYNC_BYTE])
}

/// Tell the watcher to stop.
pub(crate) fn send_stop(sender: &WakeupSocket) -> std::io::Result<()> {
    let mut writer: &WakeupSocket = sender;
    writer.write_all(&[STOP_BYTE])
}
