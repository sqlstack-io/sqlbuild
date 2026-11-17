//! SIGTERM as an interrupt: the command cancels its running statements, stops the way Ctrl+C stops it, records its end and exits with the conventional code.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use pyo3::create_exception;
use pyo3::exceptions::PyKeyboardInterrupt;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::ExecutorHost;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_host::models::WakeHandler;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_output::recall::main::write_stderr::write_stderr;

create_exception!(
    sqlbuild._native,
    CommandTerminated,
    PyKeyboardInterrupt,
    "Raised when the command receives SIGTERM; handled like Ctrl+C."
);

/// The number of SIGTERM, as POSIX defines it.
pub(crate) const SIGTERM_NUMBER: i64 = 15;
/// The number of SIGINT, which Python raises as `KeyboardInterrupt`.
pub(crate) const SIGINT_NUMBER: i64 = 2;
/// The exit code of a command stopped by SIGTERM: 128 plus the signal number.
pub(crate) const TERMINATED_EXIT_CODE: i64 = 128 + SIGTERM_NUMBER;
/// The exit code of a command stopped by Ctrl+C: 128 plus the signal number.
pub(crate) const INTERRUPTED_EXIT_CODE: i64 = 128 + SIGINT_NUMBER;
pub(crate) const TERMINATED_MESSAGE: &str = "terminated by SIGTERM";
/// `signal.SIG_DFL`, as `signal.getsignal` reports a signal Python leaves to its default action.
const DEFAULT_ACTION: i64 = 0;
/// How often the watcher looks for a termination request.
const WATCH_INTERVAL: Duration = Duration::from_millis(20);

/// The process's SIGTERM actions, registered once: take the default action while `default` is set, otherwise set `terminated`.
struct SigtermFlags {
    default: Arc<AtomicBool>,
    terminated: Arc<AtomicBool>,
}

static SIGTERM_FLAGS: OnceLock<Option<SigtermFlags>> = OnceLock::new();

/// Register the SIGTERM actions; `None` when the platform refuses them.
fn registered_flags() -> Option<&'static SigtermFlags> {
    SIGTERM_FLAGS
        .get_or_init(|| {
            let flags = SigtermFlags {
                default: Arc::new(AtomicBool::new(true)),
                terminated: Arc::new(AtomicBool::new(false)),
            };
            let signal: i32 = signal_hook::consts::SIGTERM;
            let registered =
                signal_hook::flag::register_conditional_default(signal, Arc::clone(&flags.default))
                    .and_then(|_| {
                        signal_hook::flag::register(signal, Arc::clone(&flags.terminated))
                    });
            match registered {
                Ok(_) => Some(flags),
                Err(_unsupported_signal) => None,
            }
        })
        .as_ref()
}

/// The SIGTERM handling of one command and the watcher that acts once SIGTERM arrives. While it lives, SIGTERM sets a flag; once dropped, SIGTERM takes its default action again.
pub(crate) struct TerminationHandler {
    flags: Option<&'static SigtermFlags>,
    stop: Arc<AtomicBool>,
    watcher: Option<JoinHandle<()>>,
    wake: Option<WakeHandler>,
}

impl TerminationHandler {
    /// Give SIGTERM its default action back and wait for the watcher to finish; whether SIGTERM arrived while the command ran.
    pub(crate) fn settle(&mut self, py: Python<'_>) -> bool {
        if let Some(flags) = self.flags {
            flags.default.store(true, Ordering::SeqCst);
        }
        self.stop.store(true, Ordering::SeqCst);
        if let Some(watcher) = self.watcher.take() {
            py.detach(|| drop(watcher.join()));
        }
        drop(self.wake.take());
        terminated()
    }
}

impl Drop for TerminationHandler {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(watcher) = self.watcher.take() {
            Python::attach(|py| py.detach(|| drop(watcher.join())));
        }
        drop(self.wake.take());
        if let Some(flags) = self.flags {
            flags.default.store(true, Ordering::SeqCst);
            flags.terminated.store(false, Ordering::SeqCst);
        }
    }
}

/// The signal the watcher sends the main thread: SIGINT where Python handles Ctrl+C, else the wake signal.
#[derive(Clone, Copy)]
struct MainThread {
    ident: u64,
    signal: Option<i64>,
}

/// Once SIGTERM arrives: let a second one end the process at once, cancel every running statement, and interrupt the main thread the way Ctrl+C does, where the platform can signal one thread.
fn watch(flags: &SigtermFlags, stop: &AtomicBool, main: MainThread) {
    while !stop.load(Ordering::SeqCst) {
        if flags.terminated.load(Ordering::SeqCst) {
            flags.default.store(true, Ordering::SeqCst);
            Python::attach(|py| {
                let cancelled = host_entry(py, ExecutorHost::CancelRunningStatements)
                    .and_then(|cancel| cancel.call0());
                drop(cancelled);
                if let Some(signal) = main.signal {
                    let interrupted = imported(py, "signal", "pthread_kill")
                        .and_then(|kill| kill.call1((main.ident, signal)));
                    drop(interrupted);
                }
            });
            return;
        }
        std::thread::sleep(WATCH_INTERVAL);
    }
}

/// Handle SIGTERM while the command runs: only on the main thread, and only when Python leaves SIGTERM to its default action, so an embedding application keeps its own handler.
pub(crate) fn handle_sigterm(py: Python<'_>) -> PyResult<TerminationHandler> {
    let threading: Bound<'_, PyAny> = py.import("threading")?.into_any();
    let main_thread: Bound<'_, PyAny> = threading.call_method0("main_thread")?;
    let on_main: bool = threading.call_method0("current_thread")?.is(&main_thread);
    let action = |signal: i64| -> PyResult<Bound<'_, PyAny>> {
        imported(py, "signal", "getsignal")?.call1((signal,))
    };
    let default: bool = action(SIGTERM_NUMBER)?
        .extract::<i64>()
        .is_ok_and(|action| action == DEFAULT_ACTION);
    let stop: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let Some(flags) = registered_flags().filter(|_| on_main && default) else {
        return Ok(TerminationHandler {
            flags: None,
            stop,
            watcher: None,
            wake: None,
        });
    };
    let (signal, wake): (Option<i64>, Option<WakeHandler>) =
        WakeHandler::interrupting_signal(py, || CommandTerminated::new_err(TERMINATED_MESSAGE))?;
    let main = MainThread {
        ident: native_field(&main_thread, "ident")?.extract()?,
        signal,
    };
    flags.terminated.store(false, Ordering::SeqCst);
    let watched: Arc<AtomicBool> = Arc::clone(&stop);
    let watcher: JoinHandle<()> = std::thread::Builder::new()
        .name("sqlbuild-termination".to_owned())
        .spawn(move || watch(flags, &watched, main))?;
    flags.default.store(false, Ordering::SeqCst);
    Ok(TerminationHandler {
        flags: Some(flags),
        stop,
        watcher: Some(watcher),
        wake,
    })
}

/// Whether SIGTERM arrived while the command runs.
pub(crate) fn terminated() -> bool {
    SIGTERM_FLAGS
        .get()
        .and_then(Option::as_ref)
        .is_some_and(|flags| flags.terminated.load(Ordering::SeqCst))
}

/// Whether SIGTERM arrived while the command runs, so a statement error can be reported as the termination it is.
#[pyfunction]
fn termination_requested() -> bool {
    terminated()
}

/// The signal that interrupted the command: SIGTERM once one arrived, SIGINT for any other `KeyboardInterrupt`.
pub(crate) fn interrupting_signal(py: Python<'_>, error: &PyErr) -> Option<i64> {
    if !error.is_instance_of::<PyKeyboardInterrupt>(py) {
        return None;
    }
    Some(
        if terminated() || error.is_instance_of::<CommandTerminated>(py) {
            SIGTERM_NUMBER
        } else {
            SIGINT_NUMBER
        },
    )
}

/// The exit code a run stopped by `signal` ends with.
pub(crate) fn signal_exit_code(signal: i64) -> i64 {
    if signal == SIGTERM_NUMBER {
        TERMINATED_EXIT_CODE
    } else {
        INTERRUPTED_EXIT_CODE
    }
}

/// After SIGTERM: drop the interrupt it left pending for the main thread, write the termination notice to stderr and give the conventional exit code. A stream that cannot be written changes nothing.
pub(crate) fn terminated_exit(py: Python<'_>) -> i64 {
    drop(py.check_signals());
    drop(write_stderr(py, &format!("error: {TERMINATED_MESSAGE}\n")));
    TERMINATED_EXIT_CODE
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "CommandTerminated",
        module.py().get_type::<CommandTerminated>(),
    )?;
    module.add_function(wrap_pyfunction!(termination_requested, module)?)
}
