//! `sqlbuild.recall.RecallWriter`: what a Recall replacement sink publishes through.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python, pyclass, pymethods};
use pyo3::types::{PyBytes, PyBytesMethods};
use serde_json::{Map, Value};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::recall::sinks::lifecycle_event_object;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The one stream a writer publishes to.
#[derive(Debug)]
enum Target {
    /// The events the sink published for the one it was handed.
    Lifecycle(Mutex<Vec<Map<String, Value>>>),
    /// The chunk the sink was handed, by sequence, and what it published for it.
    Output {
        sequence: u64,
        published: Mutex<Option<Vec<u8>>>,
    },
}

/// Publishes the records a Recall replacement sink keeps; valid only while the sink call runs.
#[pyclass(module = "sqlbuild._native", name = "RecallWriter", frozen)]
pub(crate) struct RecallWriter {
    target: Target,
    open: AtomicBool,
}

impl RecallWriter {
    /// A writer for one lifecycle event handed to the lifecycle replacement.
    pub(crate) fn lifecycle() -> Self {
        Self::new(Target::Lifecycle(Mutex::new(Vec::new())))
    }

    /// A writer for the output chunk with `sequence` handed to the output replacement.
    pub(crate) fn output(sequence: u64) -> Self {
        Self::new(Target::Output {
            sequence,
            published: Mutex::new(None),
        })
    }

    fn new(target: Target) -> Self {
        Self {
            target,
            open: AtomicBool::new(true),
        }
    }

    /// End the sink call: later publishing raises, and the events or output bytes published are returned.
    pub(crate) fn finish(&self) -> (Vec<Map<String, Value>>, Option<Vec<u8>>) {
        self.open.store(false, Ordering::SeqCst);
        match &self.target {
            Target::Lifecycle(published) => (
                std::mem::take(&mut *published.lock().unwrap_or_else(PoisonError::into_inner)),
                None,
            ),
            Target::Output { published, .. } => (
                Vec::new(),
                published
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .take(),
            ),
        }
    }

    fn ensure_open(&self) -> PyResult<()> {
        if self.open.load(Ordering::SeqCst) {
            return Ok(());
        }
        Err(PyValueError::new_err(
            "a RecallWriter can publish only while the sink call it was passed to runs",
        ))
    }
}

#[pymethods]
impl RecallWriter {
    /// Record `event` in Recall; for the lifecycle replacement sink.
    fn publish_lifecycle(&self, py: Python<'_>, event: &Bound<'_, PyAny>) -> PyResult<()> {
        self.ensure_open()?;
        let Target::Lifecycle(published) = &self.target else {
            return Err(PyValueError::new_err(
                "this RecallWriter belongs to a command-output sink; use publish_command_output",
            ));
        };
        if !event.is_instance(&host_entry(py, ExecutorHost::LifecycleEvent)?)? {
            return Err(PyTypeError::new_err(
                "publish_lifecycle expects a LifecycleEvent",
            ));
        }
        if let Some(object) = lifecycle_event_object(py, event)? {
            published
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(object);
        }
        Ok(())
    }

    /// Record `record.data` in Recall as the output chunk the sink was passed; at most once per call.
    fn publish_command_output(&self, py: Python<'_>, record: &Bound<'_, PyAny>) -> PyResult<()> {
        self.ensure_open()?;
        let Target::Output {
            sequence,
            published,
        } = &self.target
        else {
            return Err(PyValueError::new_err(
                "this RecallWriter belongs to a lifecycle-event sink; use publish_lifecycle",
            ));
        };
        if !record.is_instance(&host_entry(py, ExecutorHost::CommandOutputRecord)?)? {
            return Err(PyTypeError::new_err(
                "publish_command_output expects a CommandOutputRecord",
            ));
        }
        let record_sequence: u64 = native_field(record, "sequence")?.extract()?;
        if record_sequence != *sequence {
            return Err(PyValueError::new_err(format!(
                "publish_command_output expects the record passed to this call (sequence \
                 {sequence}) or a copy made with record.replace(), not sequence {record_sequence}"
            )));
        }
        let data: Vec<u8> = native_field(record, "data")?
            .downcast::<PyBytes>()
            .map_err(|_| PyTypeError::new_err("record.data must be bytes"))?
            .as_bytes()
            .to_vec();
        let mut slot = published.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.is_some() {
            return Err(PyValueError::new_err(
                "a command-output record can be published once per sink call",
            ));
        }
        *slot = Some(data);
        Ok(())
    }
}
