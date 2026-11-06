//! `OutputCaptureDispatcher`: bounded delivery of captured output under a Python condition.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python, pyclass,
    pyfunction, pymethods, wrap_pyfunction,
};
use pyo3::types::{PyDict, PyString, PyTuple};
use sqlbuild_core::command_output::constants::{
    COMMAND_OUTPUT_LOSS_RECORD_TYPE, MIN_OUTPUT_RECORD_BYTES, MIN_SHUTDOWN_TIMEOUT_SECONDS,
};
use sqlbuild_core::command_output::main::append_output::append_output;
use sqlbuild_core::command_output::main::close_output::close_output;
use sqlbuild_core::command_output::main::flush_idle_output::flush_idle_output;
use sqlbuild_core::command_output::main::output_loss_summary::output_loss_summary;
use sqlbuild_core::command_output::main::queue_output_record::queue_output_record;
use sqlbuild_core::command_output::models::{
    BufferOutcome, IdleStep, OutputBuffer, OutputQueue, OutputStream, RecordDraft, RecordPriority,
};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_threads::constants::OUTPUT_SUPPRESSED_ATTRIBUTE;
use crate::bindings::_helpers::native_threads::exit_signal::signal_interpreter_exit;
use crate::bindings::_helpers::output_capture::values::{
    freeze, object, output_error, pass_text, python_text, unencodable,
};
use crate::bindings::_helpers::records::fields::field as native_field;

const THREAD_NAME: &str = "sqlbuild-output-exporter";
/// Attribute naming a capture thread's worker, for the exit hook.
const WORKER_ATTRIBUTE: &str = "_sqlbuild_output_capture_worker";
/// How long interpreter exit waits for each stopped capture worker.
const EXIT_JOIN_SECONDS: f64 = 1.0;
const DEFAULT_QUEUE_CAPACITY: i64 = 1024;
const DEFAULT_BATCH_SIZE: i64 = 64;
const DEFAULT_MAX_RECORD_BYTES: i64 = 64 * 1024;
const DEFAULT_SHUTDOWN_TIMEOUT_SECONDS: f64 = 2.0;

/// Delivery state, read and written only while holding the dispatcher's Python condition.
struct DispatchState {
    buffer: OutputBuffer,
    queue: OutputQueue<Py<PyAny>>,
    accepting: bool,
    stopping: bool,
    force_stop: bool,
    worker_finished: bool,
    suppressed_threads: HashSet<u64>,
}

/// What the dispatcher and its worker thread share.
struct Shared {
    exporter: Py<PyAny>,
    invocation_id: Py<PyAny>,
    run_id: Py<PyAny>,
    context: Py<PyAny>,
    batch_size: usize,
    shutdown_timeout_seconds: f64,
    clock: Option<Py<PyAny>>,
    failure_callback: Option<Py<PyAny>>,
    condition: Py<PyAny>,
    started: Instant,
    state: Mutex<DispatchState>,
}

/// Turn stream fragments into bounded text chunks and export them off execution threads.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct OutputCaptureDispatcher {
    shared: Arc<Shared>,
    thread: Py<PyAny>,
}

/// The dispatcher's delivery worker, run as a Python daemon thread's target.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct OutputDeliveryWorker {
    shared: Arc<Shared>,
}

/// The Python condition, entered while alive and exited on drop.
struct Held<'py>(Bound<'py, PyAny>);

impl Drop for Held<'_> {
    fn drop(&mut self) {
        let py = self.0.py();
        drop(
            self.0
                .call_method1("__exit__", (py.None(), py.None(), py.None())),
        );
    }
}

fn stream_value(stream: OutputStream) -> &'static str {
    match stream {
        OutputStream::Stdout => "stdout",
        OutputStream::Stderr => "stderr",
    }
}

fn priority_value(priority: RecordPriority) -> &'static str {
    match priority {
        RecordPriority::Bulk => "bulk",
        RecordPriority::Terminal => "terminal",
    }
}

fn thread_ident(py: Python<'_>) -> PyResult<u64> {
    imported(py, "threading", "get_ident")?.call0()?.extract()
}

/// Whether the current thread runs a command-output sink, whose writes are never captured.
fn delivering_thread(py: Python<'_>) -> PyResult<bool> {
    imported(py, "threading", "current_thread")?
        .call0()?
        .hasattr(OUTPUT_SUPPRESSED_ATTRIBUTE)
}

/// Stop every capture worker and join it before finalization ends it inside native code.
#[pyfunction]
fn stop_output_capture_threads(py: Python<'_>) -> PyResult<()> {
    signal_interpreter_exit();
    let mut threads: Vec<Bound<'_, PyAny>> = Vec::new();
    for thread in imported(py, "threading", "enumerate")?
        .call0()?
        .try_iter()?
    {
        let thread: Bound<'_, PyAny> = thread?;
        let Ok(worker) = thread.getattr(WORKER_ATTRIBUTE) else {
            continue;
        };
        if let Ok(worker) = worker.downcast::<OutputDeliveryWorker>() {
            let shared = &worker.get().shared;
            let _held = shared.hold(py)?;
            {
                let mut state = shared.locked()?;
                state.accepting = false;
                state.stopping = true;
                state.force_stop = true;
            }
            shared.notify(py, "notify_all")?;
            threads.push(thread);
        }
    }
    for thread in threads {
        if thread.call_method0("is_alive")?.is_truthy()? {
            call_with(
                &native_field(&thread, "join")?,
                &[("timeout", object(py, EXIT_JOIN_SECONDS)?)],
            )?;
        }
    }
    Ok(())
}

impl Shared {
    fn locked(&self) -> PyResult<MutexGuard<'_, DispatchState>> {
        self.state
            .lock()
            .map_err(|_| PyRuntimeError::new_err("output capture state lock is poisoned"))
    }

    fn hold<'py>(&self, py: Python<'py>) -> PyResult<Held<'py>> {
        let condition = self.condition.bind(py).clone();
        condition.call_method0("__enter__")?;
        Ok(Held(condition))
    }

    fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    fn occurred_at<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.clock {
            Some(clock) => clock.bind(py).call0(),
            None => imported(py, "datetime", "datetime")?
                .call_method1("now", (imported(py, "datetime", "UTC")?,)),
        }
    }

    /// `CommandOutputRecord` of one draft, timestamped now.
    fn record<'py>(&self, py: Python<'py>, draft: &RecordDraft) -> PyResult<Bound<'py, PyAny>> {
        let mut fields: Vec<(&str, Bound<'py, PyAny>)> = vec![
            ("invocation_id", self.invocation_id.bind(py).clone()),
            ("run_id", self.run_id.bind(py).clone()),
            ("sequence", object(py, draft.sequence)?),
            ("occurred_at", self.occurred_at(py)?),
            (
                "stream",
                host::entry(py, ExecutorHost::CommandOutputStream)?
                    .call1((stream_value(draft.stream),))?,
            ),
            ("message", python_text(py, &draft.message)?),
            ("external_context", self.context.bind(py).clone()),
            ("chunk_index", object(py, draft.chunk_index)?),
            ("chunk_count", object(py, draft.chunk_count)?),
            (
                "priority",
                host::entry(py, ExecutorHost::OutputRecordPriority)?
                    .call1((priority_value(draft.priority),))?,
            ),
        ];
        if draft.dropped_records > 0 {
            fields.push(("record_type", object(py, COMMAND_OUTPUT_LOSS_RECORD_TYPE)?));
            fields.push(("dropped_records", object(py, draft.dropped_records)?));
        }
        call_with(
            &host::entry(py, ExecutorHost::CommandOutputRecord)?,
            &fields,
        )
    }

    /// Create and queue the records of `drafts` in order.
    fn put(&self, py: Python<'_>, drafts: &[RecordDraft]) -> PyResult<()> {
        for draft in drafts {
            let record = self.record(py, draft)?.unbind();
            let mut state = self.locked()?;
            let queue = std::mem::replace(&mut state.queue, OutputQueue::new(0));
            state.queue = queue_output_record(queue, record, draft.priority);
        }
        Ok(())
    }

    /// Queue an outcome's records, then raise its encode failure.
    fn apply(&self, py: Python<'_>, outcome: BufferOutcome, text: &[u8]) -> PyResult<()> {
        self.locked()?.buffer = outcome.buffer;
        self.put(py, &outcome.drafts)?;
        match outcome.error {
            Some(error) => Err(unencodable(py, text, &error)),
            None => Ok(()),
        }
    }

    fn take_buffer(&self) -> PyResult<OutputBuffer> {
        let mut state = self.locked()?;
        let limit = state.buffer.max_record_bytes;
        Ok(std::mem::replace(
            &mut state.buffer,
            OutputBuffer::new(limit),
        ))
    }

    fn notify(&self, py: Python<'_>, method: &str) -> PyResult<()> {
        self.condition.bind(py).call_method0(method)?;
        Ok(())
    }

    /// Wait on the condition, at most `timeout` seconds.
    fn wait(&self, py: Python<'_>, timeout: Option<f64>) -> PyResult<()> {
        let condition = self.condition.bind(py);
        match timeout {
            Some(timeout) => condition.call_method1("wait", (timeout,))?,
            None => condition.call_method0("wait")?,
        };
        Ok(())
    }

    /// The next batch, or `None` once the worker should stop.
    fn next_batch(&self, py: Python<'_>) -> PyResult<Option<Vec<Py<PyAny>>>> {
        let _held = self.hold(py)?;
        loop {
            let (idle, stopping) = {
                let state = self.locked()?;
                (state.queue.items.is_empty(), state.stopping)
            };
            if !idle || stopping {
                break;
            }
            let (buffer, step) = flush_idle_output(self.take_buffer()?, self.now());
            self.locked()?.buffer = buffer;
            match step {
                IdleStep::Wait(timeout) => self.wait(py, timeout)?,
                IdleStep::Flushed(drafts) => self.put(py, &drafts)?,
            }
        }
        let mut state = self.locked()?;
        if state.force_stop || (state.queue.items.is_empty() && state.stopping) {
            return Ok(None);
        }
        let count = self.batch_size.min(state.queue.items.len());
        Ok(Some(
            state
                .queue
                .items
                .drain(..count)
                .map(|(record, _)| record)
                .collect(),
        ))
    }

    fn report_failure(&self, py: Python<'_>, error: PyErr) {
        if let Some(callback) = &self.failure_callback {
            drop(callback.bind(py).call1((error.value(py),)));
        }
    }

    /// `_export`: deliver one batch with output from this thread excluded from capture.
    fn export(&self, py: Python<'_>, batch: Vec<Py<PyAny>>) -> PyResult<()> {
        let thread = thread_ident(py)?;
        self.locked()?.suppressed_threads.insert(thread);
        let size = u64::try_from(batch.len()).unwrap_or(u64::MAX);
        let records = PyTuple::new(py, batch)?;
        let result = self
            .exporter
            .bind(py)
            .call_method1("export_output", (records,));
        match result {
            Ok(_) => {
                let _held = self.hold(py)?;
                self.locked()?.queue.delivered += size;
            }
            Err(error) => {
                {
                    let _held = self.hold(py)?;
                    self.locked()?.queue.failed += size;
                }
                self.report_failure(py, error);
            }
        }
        self.locked()?.suppressed_threads.remove(&thread);
        Ok(())
    }

    fn run(&self, py: Python<'_>) -> PyResult<()> {
        let result = (|| -> PyResult<()> {
            while let Some(batch) = self.next_batch(py)? {
                self.export(py, batch)?;
            }
            Ok(())
        })();
        let _held = self.hold(py)?;
        self.locked()?.worker_finished = true;
        self.notify(py, "notify_all")?;
        result
    }
}

fn input_error(py: Python<'_>, message: &str) -> PyErr {
    output_error(py, ExecutorHost::OutputCaptureInputError, message)
}

#[pymethods]
impl OutputDeliveryWorker {
    fn __call__(&self, py: Python<'_>) -> PyResult<()> {
        self.shared.run(py)
    }
}

#[pymethods]
impl OutputCaptureDispatcher {
    #[new]
    #[pyo3(signature = (**options))]
    fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let options = crate::bindings::_helpers::boundary::python_calls::KeywordArguments::read(
            py,
            "OutputCaptureDispatcher",
            options,
            (
                &["exporter", "invocation_id"],
                &[
                    "run_id",
                    "external_context",
                    "queue_capacity",
                    "batch_size",
                    "max_record_bytes",
                    "shutdown_timeout_seconds",
                    "clock",
                    "failure_callback",
                ],
            ),
        )?;
        let number = |name: &str, default: i64| -> PyResult<i64> {
            let value = options.value(name)?;
            if value.is_none() {
                Ok(default)
            } else {
                value.extract()
            }
        };
        let queue_capacity = number("queue_capacity", DEFAULT_QUEUE_CAPACITY)?;
        let batch_size = number("batch_size", DEFAULT_BATCH_SIZE)?;
        let max_record_bytes = number("max_record_bytes", DEFAULT_MAX_RECORD_BYTES)?;
        let timeout_value = options.value("shutdown_timeout_seconds")?;
        let shutdown_timeout_seconds: f64 = if timeout_value.is_none() {
            DEFAULT_SHUTDOWN_TIMEOUT_SECONDS
        } else {
            timeout_value.extract()?
        };
        if queue_capacity < 1 || batch_size < 1 {
            return Err(input_error(
                py,
                "output capture queue capacity and batch size must be positive",
            ));
        }
        if max_record_bytes < MIN_OUTPUT_RECORD_BYTES
            || shutdown_timeout_seconds < MIN_SHUTDOWN_TIMEOUT_SECONDS
        {
            return Err(input_error(py, "output capture limits are invalid"));
        }
        let external = options.value("external_context")?;
        let context_source = if external.is_none() || !external.is_truthy()? {
            PyDict::new(py).into_any()
        } else {
            imported(py, "builtins", "dict")?.call1((external,))?
        };
        let context = freeze(&context_source, "external_context")?;
        let optional = |name: &str| -> PyResult<Option<Py<PyAny>>> {
            let value = options.value(name)?;
            Ok((!value.is_none()).then(|| value.unbind()))
        };
        let shared = Arc::new(Shared {
            exporter: options.value("exporter")?.unbind(),
            invocation_id: options.value("invocation_id")?.unbind(),
            run_id: options.value("run_id")?.unbind(),
            context: context.unbind(),
            batch_size: usize::try_from(batch_size).unwrap_or(usize::MAX),
            shutdown_timeout_seconds,
            clock: optional("clock")?,
            failure_callback: optional("failure_callback")?,
            condition: imported(py, "threading", "Condition")?.call0()?.unbind(),
            started: Instant::now(),
            state: Mutex::new(DispatchState {
                buffer: OutputBuffer::new(max_record_bytes),
                queue: OutputQueue::new(usize::try_from(queue_capacity).unwrap_or(usize::MAX)),
                accepting: true,
                stopping: false,
                force_stop: false,
                worker_finished: false,
                suppressed_threads: HashSet::new(),
            }),
        });
        let worker = Py::new(
            py,
            OutputDeliveryWorker {
                shared: Arc::clone(&shared),
            },
        )?;
        let worker = worker.into_bound(py).into_any();
        let thread = call_with(
            &imported(py, "threading", "Thread")?,
            &[
                ("target", worker.clone()),
                ("name", object(py, THREAD_NAME)?),
                ("daemon", object(py, true)?),
            ],
        )?;
        thread.setattr(WORKER_ATTRIBUTE, worker)?;
        thread.call_method0("start")?;
        Ok(Self {
            shared,
            thread: thread.unbind(),
        })
    }

    /// Accept one stream fragment without waiting for destination work.
    #[pyo3(signature = (*, stream, text))]
    fn append(
        &self,
        py: Python<'_>,
        stream: &Bound<'_, PyAny>,
        text: &Bound<'_, PyString>,
    ) -> PyResult<()> {
        let shared = &self.shared;
        let thread = thread_ident(py)?;
        if text.is_empty()?
            || shared.locked()?.suppressed_threads.contains(&thread)
            || delivering_thread(py)?
        {
            return Ok(());
        }
        let stream = match native_field(stream, "value")?.extract::<String>()?.as_str() {
            "stderr" => OutputStream::Stderr,
            _ => OutputStream::Stdout,
        };
        let text = pass_text(text)?;
        let _held = shared.hold(py)?;
        if !shared.locked()?.accepting {
            return Ok(());
        }
        let outcome = append_output(shared.take_buffer()?, (stream, &text), shared.now());
        let result = shared.apply(py, outcome, &text);
        shared.notify(py, "notify")?;
        result
    }

    /// Flush partial records until a fixed deadline and return without indefinite waits.
    fn close<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let shared = &self.shared;
        {
            let _held = shared.hold(py)?;
            if shared.locked()?.accepting {
                let outcome = close_output(shared.take_buffer()?, shared.now());
                shared.apply(py, outcome, &[])?;
                let summary = {
                    let mut state = shared.locked()?;
                    state.accepting = false;
                    match state.queue.dropped {
                        0 => None,
                        _ => {
                            let limit = state.buffer.max_record_bytes;
                            let buffer =
                                std::mem::replace(&mut state.buffer, OutputBuffer::new(limit));
                            let (buffer, draft) = output_loss_summary(buffer, &state.queue);
                            state.buffer = buffer;
                            Some(draft)
                        }
                    }
                };
                if let Some(draft) = summary {
                    shared.put(py, &[draft])?;
                }
                shared.locked()?.stopping = true;
                shared.notify(py, "notify_all")?;
            }
        }
        let thread = self.thread.bind(py);
        call_with(
            &native_field(thread, "join")?,
            &[(
                "timeout",
                object(py, shared.shutdown_timeout_seconds.max(0.0))?,
            )],
        )?;
        let _held = shared.hold(py)?;
        let alive = thread.call_method0("is_alive")?.is_truthy()?;
        let counts = {
            let mut state = shared.locked()?;
            if alive {
                state.force_stop = true;
                state.queue.dropped += u64::try_from(state.queue.items.len()).unwrap_or(0);
                state.queue.items.clear();
            }
            (
                state.queue.accepted,
                state.queue.delivered,
                state.queue.failed,
                state.queue.dropped,
                state.queue.items.len(),
                state.queue.capacity,
                state.worker_finished && state.queue.items.is_empty(),
            )
        };
        if alive {
            shared.notify(py, "notify_all")?;
        }
        let (accepted, delivered, failed, dropped, depth, capacity, flush_complete) = counts;
        call_with(
            &host::entry(py, ExecutorHost::CommandOutputCaptureSummary)?,
            &[
                ("accepted", object(py, accepted)?),
                ("delivered", object(py, delivered)?),
                ("failed", object(py, failed)?),
                ("dropped", object(py, dropped)?),
                ("queue_depth", object(py, depth)?),
                ("queue_capacity", object(py, capacity)?),
                ("flush_complete", object(py, flush_complete)?),
            ],
        )
    }
}

/// Add the capture classes and run `stop_output_capture_threads` at interpreter exit.
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let hook = wrap_pyfunction!(stop_output_capture_threads, module)?;
    module.add_function(hook.clone())?;
    imported(module.py(), "atexit", "register")?.call1((hook,))?;
    module.add_class::<OutputCaptureDispatcher>()?;
    module.add_class::<OutputDeliveryWorker>()
}
