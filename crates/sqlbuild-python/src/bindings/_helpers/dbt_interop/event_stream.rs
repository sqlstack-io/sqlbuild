//! Run dbt with JSON logs and render SQLBuild-styled node rows and progress while it runs.

use std::io::{self, BufReader, PipeReader};
use std::path::PathBuf;
use std::process::Child;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::sync::MutexExt;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_core::dbt_events::main::format_dbt_active_nodes::format_dbt_active_nodes;
use sqlbuild_core::dbt_process::main::dbt_process_returncode::dbt_process_returncode;
use sqlbuild_core::dbt_process::main::read_dbt_output_line::read_dbt_output_line;
use sqlbuild_core::dbt_process::main::spawn_dbt_json_process::spawn_dbt_json_process;
use sqlbuild_core::dbt_process::main::stop_dbt_process::stop_dbt_process;
use sqlbuild_core::dbt_process::models::DbtJsonProcess;

use crate::bindings::_helpers::dbt_interop::runner::{
    dbt_command_lifecycle, launch_failure, launch_os_error, report_exit,
};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_progress::main::transient_status::status_reporter::status_reporter;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_python_output::dbt_events::models::DbtEventStream;

const STREAM_OPTIONS: (&[&str], &[&str]) = (
    &["argv", "cwd", "stream", "use_color", "target_path"],
    &[
        "display_total",
        "on_node_result",
        "detail_by_unique_id",
        "enable_status",
    ],
);
/// A running node: unique id, display name and start offset in seconds.
type ActiveNode = (String, String, f64);
type LineResult = io::Result<Option<String>>;

const WAITING_MESSAGE: &str = "Waiting for dbt node output...";
/// How often the running-node progress line refreshes while dbt is silent.
const STATUS_REFRESH: Duration = Duration::from_secs(1);
/// How long cleanup waits for the progress refresher to stop.
const REFRESHER_JOIN: Duration = Duration::from_secs(1);
/// How often a blocked read rechecks for interrupt signals.
const SIGNAL_POLL: Duration = Duration::from_millis(100);

/// The live progress line and the running nodes it lists, in first-start order.
struct StatusState {
    status: Option<Py<PyAny>>,
    active_nodes: Vec<ActiveNode>,
}

/// What the read loop and the progress refresher share.
struct Shared {
    state: Mutex<StatusState>,
    stopped: Mutex<bool>,
    wake: Condvar,
    epoch: Instant,
}

/// The output stream, its colour and tty flags, and whether progress lines are drawn.
struct Output<'py> {
    stream: Bound<'py, PyAny>,
    use_color: bool,
    enable_status: bool,
}

/// Run dbt and render SQLBuild-styled rows from JSON events; returns the exit code and results.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn execute_dbt_json_event_stream<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "execute_dbt_json_event_stream", options, STREAM_OPTIONS)?;
    let enable_status: bool =
        !arguments.passed("enable_status")? || arguments.flag("enable_status")?;
    let output: Output<'py> = Output {
        stream: arguments.value("stream")?,
        use_color: arguments.flag("use_color")?,
        enable_status,
    };
    let events: Bound<'py, PyAny> = {
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        keywords.set_item("display_total", arguments.value("display_total")?)?;
        keywords.set_item(
            "detail_by_unique_id",
            arguments.value("detail_by_unique_id")?,
        )?;
        keywords.set_item("use_color", output.use_color)?;
        DbtEventStream::new(py, Some(keywords.clone()))
            .and_then(|value| pyo3::Bound::new(py, value))
            .map(pyo3::Bound::into_any)?
    };
    let argv: Vec<String> = arguments.value("argv")?.extract()?;
    let cwd: Option<PathBuf> = arguments.value("cwd")?.extract()?;
    let target_path: Option<PathBuf> = arguments.value("target_path")?.extract()?;
    let on_node_result: Bound<'py, PyAny> = arguments.value("on_node_result")?;
    let lifecycle: Bound<'py, PyAny> = dbt_command_lifecycle(py)?;
    let (returncode, results) = within(&lifecycle, |lifecycle| {
        let (returncode, results, process_id) = streamed(
            (&argv, cwd.as_ref(), target_path.as_ref()),
            &output,
            (&events, &on_node_result),
        )?;
        report_exit(lifecycle, returncode, process_id)?;
        Ok((returncode, results))
    })?;
    PyTuple::new(
        py,
        [
            returncode.into_bound_py_any(py)?,
            PyTuple::new(py, results)?.into_any(),
        ],
    )
}

/// Launch dbt, stream its lines, then always stop the refresher, the status and the process.
fn streamed<'py>(
    (argv, cwd, target_path): (&[String], Option<&PathBuf>, Option<&PathBuf>),
    output: &Output<'py>,
    (events, on_node_result): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<(i64, Vec<Bound<'py, PyAny>>, Option<u32>)> {
    let py: Python<'py> = output.stream.py();
    let DbtJsonProcess {
        child,
        output: lines,
    } = spawn_dbt_json_process(
        argv,
        (cwd.map(PathBuf::as_path), target_path.map(PathBuf::as_path)),
    )
    .map_err(|error| {
        launch_failure(
            py,
            launch_os_error(py, &error, (argv, cwd.map(PathBuf::as_path))),
        )
    })?;
    let mut child: Child = child;
    let tty: bool = output.stream.call_method0("isatty")?.is_truthy()?;
    let shared: Arc<Shared> = Arc::new(Shared {
        state: Mutex::new(StatusState {
            status: None,
            active_nodes: Vec::new(),
        }),
        stopped: Mutex::new(false),
        wake: Condvar::new(),
        epoch: Instant::now(),
    });
    let mut refresher: Option<(JoinHandle<()>, Receiver<()>)> = None;
    let mut waited: bool = false;
    let result: PyResult<(i64, Vec<Bound<'py, PyAny>>)> = (|| {
        if output.enable_status && tty {
            let status: Bound<'py, PyAny> = status_reporter(&output.stream, output.use_color)?;
            status.call_method1("start", (WAITING_MESSAGE,))?;
            locked(py, &shared)?.status = Some(status.unbind());
            refresher = Some(start_refresher(
                &shared,
                (output.stream.clone().unbind(), output.use_color),
            ));
        }
        let mut results: Vec<Bound<'py, PyAny>> = Vec::new();
        let received: Mutex<Receiver<io::Result<Option<String>>>> = Mutex::new(start_reader(lines));
        while let Some(line) = next_line(py, &received)? {
            handle_line(
                &line,
                output,
                (events, on_node_result),
                (&shared, tty, &mut results),
            )?;
        }
        let returncode: i64 = py
            .compiler_detach(|| Ok(dbt_process_returncode(&mut child)))
            .map_err(PyRuntimeError::new_err)?
            .map_err(PyErr::from)?;
        waited = true;
        py.check_signals()?;
        Ok((returncode, results))
    })();
    let process_id: u32 = cleanup(py, &shared, refresher, (child, waited));
    let (returncode, results) = result?;
    Ok((returncode, results, Some(process_id)))
}

/// One dbt output line: progress message, node row, results and the running-node status.
fn handle_line<'py>(
    line: &str,
    output: &Output<'py>,
    (events, on_node_result): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (shared, tty, results): (&Shared, bool, &mut Vec<Bound<'py, PyAny>>),
) -> PyResult<()> {
    let py: Python<'py> = events.py();
    let update: Bound<'py, PyAny> = events.call_method1("consume", (line,))?;
    if update.is_none() {
        return Ok(());
    }
    let message: Bound<'py, PyAny> = native_field(&update, "status_message")?;
    if !message.is_none() && output.enable_status {
        let mut state = locked(py, shared)?;
        let status: Bound<'py, PyAny> = match state.status.take() {
            Some(status) => {
                let status: Bound<'py, PyAny> = status.into_bound(py);
                status.call_method1("update", (&message,))?;
                status
            }
            None => {
                let status: Bound<'py, PyAny> = status_reporter(&output.stream, output.use_color)?;
                status.call_method1("start", (&message,))?;
                status
            }
        };
        state.status = Some(status.unbind());
    }
    let result: Bound<'py, PyAny> = native_field(&update, "result")?;
    let row: Bound<'py, PyAny> = native_field(&update, "row")?;
    if result.is_none() || row.is_none() {
        return Ok(());
    }
    close_status(py, shared)?;
    let finished: bool = native_field(&update, "finished")?.is_truthy()?;
    let unique_id: String = native_field(&result, "unique_id")?.extract()?;
    if finished {
        locked(py, shared)?
            .active_nodes
            .retain(|(id, _, _)| *id != unique_id);
        results.push(result.clone());
        if !on_node_result.is_none() {
            on_node_result.call1((&result,))?;
        }
    }
    output.stream.call_method1("write", (&row,))?;
    output.stream.call_method0("flush")?;
    if output.enable_status {
        let mut state = locked(py, shared)?;
        if !finished {
            let started: f64 = shared.epoch.elapsed().as_secs_f64();
            let name: String = native_field(&result, "node_name")?.extract()?;
            match state
                .active_nodes
                .iter_mut()
                .find(|(id, _, _)| *id == unique_id)
            {
                Some(node) => *node = (unique_id, name, started),
                None => state.active_nodes.push((unique_id, name, started)),
            }
        }
        let current: Option<Py<PyAny>> = state.status.take();
        state.status = refreshed_status(
            py,
            (&output.stream, output.use_color, tty),
            shared,
            (current, &state.active_nodes),
        )?;
    }
    Ok(())
}

/// The redrawn running-node line; with no running node or no tty the status is dropped.
fn refreshed_status(
    py: Python<'_>,
    (stream, use_color, tty): (&Bound<'_, PyAny>, bool, bool),
    shared: &Shared,
    (current, active_nodes): (Option<Py<PyAny>>, &[ActiveNode]),
) -> PyResult<Option<Py<PyAny>>> {
    if !tty || active_nodes.is_empty() {
        return Ok(None);
    }
    let nodes: Vec<(String, f64)> = active_nodes
        .iter()
        .map(|(_, name, started)| (name.clone(), *started))
        .collect();
    let message: String = format_dbt_active_nodes(&nodes, shared.epoch.elapsed().as_secs_f64());
    let status: Bound<'_, PyAny> = match current {
        Some(status) => {
            let status = status.into_bound(py);
            status.call_method1("update", (message,))?;
            status
        }
        None => {
            let status = status_reporter(stream, use_color)?;
            status.call_method1("start", (message,))?;
            status
        }
    };
    Ok(Some(status.unbind()))
}

fn close_status(py: Python<'_>, shared: &Shared) -> PyResult<()> {
    let mut state = locked(py, shared)?;
    if let Some(status) = state.status.take() {
        status.bind(py).call_method0("close")?;
    }
    Ok(())
}

fn locked<'a>(
    py: Python<'_>,
    shared: &'a Shared,
) -> PyResult<std::sync::MutexGuard<'a, StatusState>> {
    shared
        .state
        .lock_py_attached(py)
        .map_err(|_| PyRuntimeError::new_err("dbt status state is poisoned"))
}

/// Read output lines on a thread so the waiting caller can notice interrupt signals.
fn start_reader(mut lines: BufReader<PipeReader>) -> Receiver<io::Result<Option<String>>> {
    let (sender, receiver): (Sender<LineResult>, Receiver<LineResult>) = channel();
    std::thread::spawn(move || {
        loop {
            let line: io::Result<Option<String>> = read_dbt_output_line(&mut lines);
            let last: bool = !matches!(line, Ok(Some(_)));
            if sender.send(line).is_err() || last {
                break;
            }
        }
    });
    receiver
}

/// The next line, raising pending interrupt signals while dbt is silent.
fn next_line(
    py: Python<'_>,
    received: &Mutex<Receiver<io::Result<Option<String>>>>,
) -> PyResult<Option<String>> {
    loop {
        let next: Result<io::Result<Option<String>>, RecvTimeoutError> = py.detach(|| {
            received
                .lock()
                .map_or(Err(RecvTimeoutError::Disconnected), |receiver| {
                    receiver.recv_timeout(SIGNAL_POLL)
                })
        });
        match next {
            Ok(line) => {
                py.check_signals()?;
                return line.map_err(PyErr::from);
            }
            Err(RecvTimeoutError::Timeout) => py.check_signals()?,
            Err(RecvTimeoutError::Disconnected) => return Ok(None),
        }
    }
}

/// Refresh the running-node line every second until stopped; reports when it has ended.
fn start_refresher(
    shared: &Arc<Shared>,
    (stream, use_color): (Py<PyAny>, bool),
) -> (JoinHandle<()>, Receiver<()>) {
    let (done, finished): (Sender<()>, Receiver<()>) = channel();
    let shared: Arc<Shared> = Arc::clone(shared);
    let handle: JoinHandle<()> = std::thread::spawn(move || {
        while !wait_stopped(&shared) {
            let refreshed: PyResult<()> = Python::attach(|py| {
                let mut state = locked(py, &shared)?;
                let current: Option<Py<PyAny>> = state.status.take();
                state.status = refreshed_status(
                    py,
                    (stream.bind(py), use_color, true),
                    &shared,
                    (current, &state.active_nodes),
                )?;
                Ok(())
            });
            if refreshed.is_err() {
                break;
            }
        }
        let _sent: Result<(), std::sync::mpsc::SendError<()>> = done.send(());
    });
    (handle, finished)
}

/// Wait one refresh period; whether cleanup asked the refresher to stop.
fn wait_stopped(shared: &Shared) -> bool {
    let Ok(stopped) = shared.stopped.lock() else {
        return true;
    };
    if *stopped {
        return true;
    }
    match shared.wake.wait_timeout(stopped, STATUS_REFRESH) {
        Ok((stopped, _)) => *stopped,
        Err(_) => true,
    }
}

/// Stop the refresher, close the status and stop dbt when it was not waited for.
fn cleanup(
    py: Python<'_>,
    shared: &Shared,
    refresher: Option<(JoinHandle<()>, Receiver<()>)>,
    (child, waited): (Child, bool),
) -> u32 {
    if let Ok(mut stopped) = shared.stopped.lock() {
        *stopped = true;
    }
    shared.wake.notify_all();
    if let Some((handle, finished)) = refresher {
        let finished: Mutex<Receiver<()>> = Mutex::new(finished);
        let ended: bool = py.detach(|| match finished.lock() {
            Ok(receiver) => match receiver.recv_timeout(REFRESHER_JOIN) {
                Ok(()) => true,
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => false,
            },
            Err(_) => false,
        });
        if ended {
            let _joined: std::thread::Result<()> = handle.join();
        }
    }
    let _closed: PyResult<()> = close_status(py, shared);
    if waited {
        return child.id();
    }
    py.detach(|| stop_dbt_process(child, terminate)).id()
}

/// Ask a process to stop as `Popen.terminate` does.
fn terminate(process_id: u32) {
    let _requested: PyResult<()> = Python::attach(|py| {
        if cfg!(windows) {
            return Ok(());
        }
        let signal: Bound<'_, PyAny> = imported(py, "signal", "SIGTERM")?;
        imported(py, "os", "kill")?.call1((process_id, signal))?;
        Ok(())
    });
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(execute_dbt_json_event_stream, module)?)
}
