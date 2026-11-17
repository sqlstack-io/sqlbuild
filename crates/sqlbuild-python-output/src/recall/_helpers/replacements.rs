//! Recall replacement sinks: the one lifecycle and one command-output sink that decide what Recall stores.

use std::cell::Cell;
use std::sync::Arc;

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBytes, PyDict, PyDictMethods, PyString};
use sqlbuild_core::command_output::constants::RECALL_REPLACEMENT_SCHEMA_VERSION;
use sqlbuild_core::command_output::main::ansi_free_text::ansi_free_text;
use sqlbuild_core::event_export::models::ExporterFilter;
use sqlbuild_host::entries::models::{ExecutorHost, UserCodeHost};
use sqlbuild_recall::records::models::{OutputChunk, OutputStream};
use sqlbuild_recall::writer::models::{ReplacedEvents, ReplacedOutput};
use sqlbuild_recall::writer::types::{OutputReplacement, PendingLifecycleEvent};

use crate::output_capture::main::invocation_context::invocation_context;
use crate::recall::constants::EVENT_PARAMETER;
use crate::recall::constants::RECORD_PARAMETER;
use crate::recall::constants::UTC_OFFSET;
use crate::recall::constants::UTC_SUFFIX;
use crate::recall::models::RecallWriter;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_threads::constants::OUTPUT_SUPPRESSED_ATTRIBUTE;

thread_local! {
    /// Set while this thread runs a replacement sink, whose own writes Recall never records.
    static IN_REPLACEMENT_SINK: Cell<bool> = const { Cell::new(false) };
}

/// Whether the current thread is running a replacement sink.
pub(crate) fn in_replacement_sink() -> bool {
    IN_REPLACEMENT_SINK.with(Cell::get)
}

/// Run `call` as a replacement sink on the delivery thread: its writes are neither recorded nor handed to command-output sinks.
fn as_replacement_sink<T>(call: impl FnOnce(Python<'_>) -> PyResult<T>, failed: T) -> T {
    IN_REPLACEMENT_SINK.with(|flag| flag.set(true));
    let result = Python::attach(|py| {
        if let Ok(thread) =
            imported(py, "threading", "current_thread").and_then(|current| current.call0())
        {
            drop(thread.setattr(OUTPUT_SUPPRESSED_ATTRIBUTE, true));
        }
        call(py)
    })
    .unwrap_or(failed);
    IN_REPLACEMENT_SINK.with(|flag| flag.set(false));
    result
}

/// The lifecycle replacement: the sink function, the parameter its writer goes in, and its event filter.
#[derive(Debug)]
pub(crate) struct LifecycleReplacement {
    pub(crate) function: Py<PyAny>,
    pub(crate) writer_parameter: String,
    pub(crate) filter: ExporterFilter,
}

/// `function(**{parameter: record, writer_parameter: writer})` through the user-code shim; whether it raised.
fn invoke(
    py: Python<'_>,
    function: &Py<PyAny>,
    (parameter, record): (&str, &Bound<'_, PyAny>),
    (writer_parameter, writer): (&str, Bound<'_, PyAny>),
) -> PyResult<bool> {
    let arguments = PyDict::new(py);
    arguments.set_item(parameter, record)?;
    arguments.set_item(writer_parameter, writer)?;
    call_with(
        &host_entry(py, UserCodeHost::InvokeRecallSink)?,
        &[
            ("function", function.bind(py).clone()),
            ("arguments", arguments.into_any()),
        ],
    )?
    .extract()
}

/// The `(sink, writer_parameter)` pair discovery found, or `None`.
pub(crate) fn discovered<'py>(
    pair: &Bound<'py, PyAny>,
) -> PyResult<Option<(Bound<'py, PyAny>, String)>> {
    if pair.is_none() {
        return Ok(None);
    }
    let (sink, writer_parameter): (Bound<'py, PyAny>, String) = pair.extract()?;
    Ok(Some((sink, writer_parameter)))
}

/// One lifecycle event queued for the lifecycle replacement.
#[derive(Debug)]
pub(crate) struct PendingEvent {
    pub(crate) replacement: Arc<LifecycleReplacement>,
    pub(crate) event: Py<PyAny>,
}

impl PendingEvent {
    pub(crate) fn call(&self, py: Python<'_>) -> PyResult<ReplacedEvents> {
        let writer = Bound::new(py, RecallWriter::lifecycle())?;
        let raised: bool = invoke(
            py,
            &self.replacement.function,
            (EVENT_PARAMETER, self.event.bind(py)),
            (
                &self.replacement.writer_parameter,
                writer.clone().into_any(),
            ),
        )?;
        let (events, _) = writer.get().finish();
        Ok(if raised {
            ReplacedEvents::Failed
        } else {
            ReplacedEvents::Published(events)
        })
    }
}

impl PendingLifecycleEvent for PendingEvent {
    fn replace(self: Box<Self>) -> ReplacedEvents {
        as_replacement_sink(|py| self.call(py), ReplacedEvents::Failed)
    }
}

/// The output replacement, called on the Recall delivery thread for each chunk.
pub(crate) struct OutputSink {
    pub(crate) function: Py<PyAny>,
    pub(crate) writer_parameter: String,
    /// Whether the sink takes stdout and stderr chunks; others are filtered.
    pub(crate) streams: [bool; 2],
    pub(crate) invocation_id: String,
}

impl std::fmt::Debug for OutputSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OutputSink")
            .field("writer_parameter", &self.writer_parameter)
            .field("streams", &self.streams)
            .finish_non_exhaustive()
    }
}

impl OutputSink {
    /// The schema-2 `CommandOutputRecord` of `chunk`.
    pub(crate) fn record<'py>(
        &self,
        py: Python<'py>,
        chunk: &OutputChunk,
    ) -> PyResult<Bound<'py, PyAny>> {
        let occurred_at: Bound<'py, PyAny> = imported(py, "datetime", "datetime")?.call_method1(
            "fromisoformat",
            (chunk.occurred_at.replace(UTC_SUFFIX, UTC_OFFSET),),
        )?;
        let context: Bound<'py, PyAny> =
            invocation_context(py, None).unwrap_or_else(|_| PyDict::new(py).into_any());
        call_with(
            &host_entry(py, ExecutorHost::CommandOutputRecord)?,
            &[
                (
                    "invocation_id",
                    PyString::new(py, &self.invocation_id).into_any(),
                ),
                ("sequence", chunk.sequence.into_bound_py_any(py)?),
                ("occurred_at", occurred_at),
                (
                    "stream",
                    host_entry(py, ExecutorHost::CommandOutputStream)?
                        .call1((chunk.stream.as_str(),))?,
                ),
                (
                    "message",
                    PyString::new(py, &ansi_free_text(&chunk.data)).into_any(),
                ),
                ("external_context", context),
                ("data", PyBytes::new(py, &chunk.data).into_any()),
                (
                    "schema_version",
                    RECALL_REPLACEMENT_SCHEMA_VERSION.into_bound_py_any(py)?,
                ),
            ],
        )
    }

    pub(crate) fn call(&self, py: Python<'_>, chunk: &OutputChunk) -> PyResult<ReplacedOutput> {
        let record = self.record(py, chunk)?;
        let writer = Bound::new(py, RecallWriter::output(chunk.sequence))?;
        let raised: bool = invoke(
            py,
            &self.function,
            (RECORD_PARAMETER, &record),
            (&self.writer_parameter, writer.clone().into_any()),
        )?;
        let (_, published) = writer.get().finish();
        Ok(match (raised, published) {
            (true, _) => ReplacedOutput::Failed,
            (false, Some(data)) => ReplacedOutput::Published(data),
            (false, None) => ReplacedOutput::Filtered,
        })
    }
}

impl OutputReplacement for OutputSink {
    fn replace(&self, chunk: &OutputChunk) -> ReplacedOutput {
        if !self.streams[usize::from(chunk.stream == OutputStream::Stderr)] {
            return ReplacedOutput::Filtered;
        }
        as_replacement_sink(|py| self.call(py, chunk), ReplacedOutput::Failed)
    }
}
