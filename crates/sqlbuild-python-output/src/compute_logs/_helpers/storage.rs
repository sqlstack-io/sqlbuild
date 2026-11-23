//! `LocalFilesystemComputeLogStorage`: the project-local compute log backend.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Seek, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyRef, PyResult, Python, pyclass, pymethods};
use pyo3::types::{PyBytes, PyBytesMethods, PyDict, PyString, PyStringMethods, PyTuple};
use sqlbuild_cli::compute_logs::constants::{
    DEFAULT_RETENTION_COUNT, MAX_READ_BYTES, STREAM_FILE_NAMES,
};
use sqlbuild_cli::compute_logs::errors::{ComputeLogError, ComputeLogErrorKind};
use sqlbuild_cli::compute_logs::main::capture_inventory::capture_inventory;
use sqlbuild_cli::compute_logs::main::capture_is_complete::capture_is_complete;
use sqlbuild_cli::compute_logs::main::check_completion::check_completion;
use sqlbuild_cli::compute_logs::main::complete_capture::complete_capture;
use sqlbuild_cli::compute_logs::main::delete_capture::delete_capture;
use sqlbuild_cli::compute_logs::main::open_log_root::open_log_root;
use sqlbuild_cli::compute_logs::main::prune_captures::prune_captures;
use sqlbuild_cli::compute_logs::main::publish_capture::publish_capture;
use sqlbuild_cli::compute_logs::main::read_capture_document::read_capture_document;
use sqlbuild_cli::compute_logs::main::read_stream_chunk::read_stream_chunk;
use sqlbuild_cli::compute_logs::main::stream_byte_counts::stream_byte_counts;
use sqlbuild_cli::compute_logs::main::validate_capture_id::validate_capture_id;
use sqlbuild_cli::compute_logs::models::CaptureDocument;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_recall::writer::models::RecallSession;

use crate::compute_logs::_helpers::conversion::capture_id;
use crate::compute_logs::_helpers::conversion::document;
use crate::compute_logs::_helpers::conversion::inventory_item;
use crate::compute_logs::_helpers::conversion::is_exact_int;
use crate::compute_logs::_helpers::conversion::metadata_model;
use crate::compute_logs::_helpers::conversion::rejection;
use crate::compute_logs::_helpers::conversion::storage_error;
use crate::compute_logs::_helpers::conversion::stream_index;
use crate::recall::constants::STDOUT_STREAM_INDEX;
use crate::recall::main::forward_diagnostic::forward_diagnostic;
use crate::recall::main::forward_output::forward_output;
use crate::recall::models::RecallLifecycleSink;
use sqlbuild_orchestration::run_output::main::forward_output::forward_output as forward_run_output;
use sqlbuild_orchestration::run_output::main::wait_for_output_room::wait_for_output_room;
use sqlbuild_orchestration::run_output::models::OutputStream;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

const DIAGNOSTICS_STREAM: usize = 2;

/// Open stream files of the captures this instance started, and whether it is closed.
#[derive(Default)]
struct StorageState {
    writers: HashMap<String, [File; 3]>,
    closed: bool,
}

/// Ordered fsync capture storage subject to underlying filesystem and hardware guarantees.
#[pyclass(module = "sqlbuild._native", frozen, subclass)]
pub(crate) struct LocalFilesystemComputeLogStorage {
    root: PathBuf,
    retention_count: Option<i64>,
    state: Mutex<StorageState>,
    /// The invocation's Recall session, fed every byte and diagnostic the capture accepts.
    recall: OnceLock<Arc<RecallSession>>,
}

/// The stdout, stderr and diagnostics byte counts of open streams, as `tell()` reports them.
fn stream_positions(streams: &[File; 3]) -> std::io::Result<[i64; 3]> {
    let mut counts: [i64; 3] = [0; 3];
    for (count, mut stream) in counts.iter_mut().zip(streams.iter()) {
        *count = i64::try_from(stream.stream_position()?).unwrap_or(i64::MAX);
    }
    Ok(counts)
}

fn not_active(py: Python<'_>) -> pyo3::PyErr {
    rejection(
        py,
        ComputeLogErrorKind::State,
        "capture is not active in this storage instance",
    )
}

fn checked<T>(py: Python<'_>, result: Result<T, ComputeLogError>) -> PyResult<T> {
    result.map_err(|error| storage_error(py, error))
}

/// `value` as an integer when it is exactly an `int` that fits, else `fallback`.
fn exact_count(value: &Bound<'_, PyAny>, fallback: i64) -> i64 {
    if !is_exact_int(value) {
        return fallback;
    }
    value.extract().unwrap_or(fallback)
}

impl LocalFilesystemComputeLogStorage {
    /// Callers wait for this lock holding the GIL, so never run Python while holding it.
    fn locked(&self) -> PyResult<MutexGuard<'_, StorageState>> {
        self.state
            .lock()
            .map_err(|_| PyRuntimeError::new_err("compute log storage lock is poisoned"))
    }

    fn ensure_open(&self, py: Python<'_>) -> PyResult<()> {
        if self.locked()?.closed {
            return Err(rejection(
                py,
                ComputeLogErrorKind::State,
                "compute log storage is closed",
            ));
        }
        Ok(())
    }

    fn append_bytes(
        &self,
        py: Python<'_>,
        invocation_id: &str,
        stream: usize,
        data: &[u8],
    ) -> PyResult<()> {
        let mut state = self.locked()?;
        let Some(writers) = state.writers.get_mut(invocation_id) else {
            drop(state);
            return Err(not_active(py));
        };
        writers[stream].write_all(data)?;
        self.forward_to_recall(invocation_id, stream, data);
        drop(state);
        if stream != DIAGNOSTICS_STREAM {
            py.detach(wait_for_output_room);
        }
        Ok(())
    }

    /// Hand bytes the capture just accepted to Recall and the run output while its lock is held, so all keep one order.
    fn forward_to_recall(&self, invocation_id: &str, stream: usize, data: &[u8]) {
        let session = self.recall.get().map(Arc::as_ref);
        if stream != DIAGNOSTICS_STREAM {
            forward_output(session, invocation_id, (stream, data));
            forward_run_output(
                if stream == STDOUT_STREAM_INDEX {
                    OutputStream::Stdout
                } else {
                    OutputStream::Stderr
                },
                data,
            );
            return;
        }
        if let Ok(line) = std::str::from_utf8(data) {
            forward_diagnostic(session, invocation_id, line.trim_end_matches('\n'));
        }
    }

    /// The capture directory once final metadata matches the open, synced streams.
    fn checked_completion(
        &self,
        state: &StorageState,
        document: &CaptureDocument,
    ) -> Result<PathBuf, ComputeLogError> {
        let streams = state.writers.get(&document.invocation_id);
        let counts = match streams {
            Some(streams) => Some(stream_positions(streams)?),
            None => None,
        };
        let directory = check_completion(&self.root, document, counts)?;
        for stream in streams.into_iter().flatten() {
            stream.sync_all()?;
        }
        Ok(directory)
    }

    fn byte_counts(&self, py: Python<'_>, invocation_id: &Bound<'_, PyAny>) -> PyResult<[u64; 3]> {
        let invocation_id = capture_id(invocation_id)?;
        checked(py, stream_byte_counts(&self.root, &invocation_id))
    }
}

#[pymethods]
impl LocalFilesystemComputeLogStorage {
    #[new]
    #[pyo3(signature = (*, project_dir, root = None, retention_count = Some(DEFAULT_RETENTION_COUNT as i64)))]
    fn new(
        py: Python<'_>,
        project_dir: PathBuf,
        root: Option<PathBuf>,
        retention_count: Option<i64>,
    ) -> PyResult<Self> {
        let root = checked(
            py,
            open_log_root(&project_dir, root.as_deref(), retention_count),
        )?;
        Ok(Self {
            root,
            retention_count,
            state: Mutex::new(StorageState::default()),
            recall: OnceLock::new(),
        })
    }

    /// Feed every accepted byte and diagnostic to the invocation's Recall session.
    fn attach_recall(&self, recall: PyRef<'_, RecallLifecycleSink>) {
        drop(self.recall.set(Arc::clone(&recall.session)));
    }

    /// Return the resolved configured log root.
    #[getter]
    pub(crate) fn root<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        imported(py, "pathlib", "Path")?.call1((self.root.as_path(),))
    }

    /// Create one incomplete capture and open its serialized append streams.
    fn start_capture(&self, py: Python<'_>, metadata: &Bound<'_, PyAny>) -> PyResult<()> {
        self.ensure_open(py)?;
        let document = document(metadata)?;
        let published = {
            let mut state = self.locked()?;
            publish_capture(&self.root, &document).map(|writers| {
                drop(
                    state
                        .writers
                        .insert(document.invocation_id.clone(), writers),
                )
            })
        };
        checked(py, published)
    }

    /// Append exact raw bytes and make them visible to concurrent readers.
    #[pyo3(signature = (*, invocation_id, stream, data))]
    fn append(
        &self,
        py: Python<'_>,
        invocation_id: &Bound<'_, PyAny>,
        stream: &Bound<'_, PyAny>,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.ensure_open(py)?;
        let invocation_id = capture_id(invocation_id)?;
        checked(py, validate_capture_id(&invocation_id))?;
        let stream = stream_index(stream)?;
        let Ok(bytes) = data.downcast::<PyBytes>() else {
            return Err(rejection(
                py,
                ComputeLogErrorKind::State,
                "compute log append data must be bytes",
            ));
        };
        self.append_bytes(py, &invocation_id, stream, bytes.as_bytes())
    }

    /// Append one validated deterministic UTF-8 diagnostic JSON line.
    #[pyo3(signature = (*, invocation_id, record))]
    fn append_diagnostic(
        &self,
        py: Python<'_>,
        invocation_id: &Bound<'_, PyAny>,
        record: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if !record.is_instance(&host_entry(py, ExecutorHost::DiagnosticLog)?)? {
            return Err(rejection(
                py,
                ComputeLogErrorKind::State,
                "diagnostic append requires DiagnosticLog",
            ));
        }
        let encoded: Bound<'_, PyAny> =
            host_entry(py, ExecutorHost::DiagnosticLogToJson)?.call1((record,))?;
        let text: &str = encoded.downcast::<PyString>()?.to_str()?;
        let mut line: Vec<u8> = text.as_bytes().to_vec();
        line.push(b'\n');
        self.ensure_open(py)?;
        let invocation_id = capture_id(invocation_id)?;
        checked(py, validate_capture_id(&invocation_id))?;
        self.append_bytes(py, &invocation_id, DIAGNOSTICS_STREAM, &line)
    }

    /// Read at most the public byte limit from an exact stream offset.
    #[pyo3(signature = (**options))]
    fn read<'py>(
        &self,
        py: Python<'py>,
        options: Option<Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let options = KeywordArguments::read(
            py,
            "read",
            options,
            (&["invocation_id", "stream"], &["cursor", "max_bytes"]),
        )?;
        let invocation_id = capture_id(&options.value("invocation_id")?)?;
        checked(py, validate_capture_id(&invocation_id))?;
        let stream = stream_index(&options.value("stream")?)?;
        let cursor = match options.contains("cursor")? {
            true => exact_count(&options.value("cursor")?, -1),
            false => 0,
        };
        let max_bytes = match options.contains("max_bytes")? {
            true => exact_count(&options.value("max_bytes")?, 0),
            false => i64::try_from(MAX_READ_BYTES).unwrap_or(i64::MAX),
        };
        let chunk = checked(
            py,
            read_stream_chunk(
                &self.root,
                (&invocation_id, STREAM_FILE_NAMES[stream]),
                cursor,
                max_bytes,
            ),
        )?;
        call_with(
            &host_entry(py, ExecutorHost::ComputeLogReadChunk)?,
            &[
                ("data", PyBytes::new(py, &chunk.data).into_any()),
                ("next_cursor", chunk.next_cursor.into_bound_py_any(py)?),
                ("is_complete", chunk.is_complete.into_bound_py_any(py)?),
            ],
        )
    }

    /// Read the currently atomically published metadata document.
    #[pyo3(name = "get_metadata", signature = (*, invocation_id))]
    fn metadata<'py>(
        &self,
        py: Python<'py>,
        invocation_id: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let invocation_id = capture_id(invocation_id)?;
        let (document, _) = checked(py, read_capture_document(&self.root, &invocation_id))?;
        metadata_model(py, &document)
    }

    /// Return the current raw byte count for one stream.
    #[pyo3(name = "get_byte_count", signature = (*, invocation_id, stream))]
    fn byte_count(
        &self,
        py: Python<'_>,
        invocation_id: &Bound<'_, PyAny>,
        stream: &Bound<'_, PyAny>,
    ) -> PyResult<u64> {
        let stream = stream_index(stream)?;
        Ok(self.byte_counts(py, invocation_id)?[stream])
    }

    /// Return one lock-consistent snapshot of all stream sizes.
    #[pyo3(name = "get_byte_counts", signature = (*, invocation_id))]
    fn all_byte_counts<'py>(
        &self,
        py: Python<'py>,
        invocation_id: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let [stdout_bytes, stderr_bytes, diagnostics_bytes] =
            self.byte_counts(py, invocation_id)?;
        call_with(
            &host_entry(py, ExecutorHost::CaptureByteCounts)?,
            &[
                ("stdout_bytes", stdout_bytes.into_bound_py_any(py)?),
                ("stderr_bytes", stderr_bytes.into_bound_py_any(py)?),
                (
                    "diagnostics_bytes",
                    diagnostics_bytes.into_bound_py_any(py)?,
                ),
            ],
        )
    }

    /// Return capture-close evidence without asserting execution lifecycle status.
    #[pyo3(name = "is_complete", signature = (*, invocation_id))]
    fn complete_marker(&self, py: Python<'_>, invocation_id: &Bound<'_, PyAny>) -> PyResult<bool> {
        let invocation_id = capture_id(invocation_id)?;
        checked(py, capture_is_complete(&self.root, &invocation_id))
    }

    /// Flush, fsync, and close streams before publishing final metadata and marker.
    fn mark_complete(&self, py: Python<'_>, metadata: &Bound<'_, PyAny>) -> PyResult<()> {
        self.ensure_open(py)?;
        let document = document(metadata)?;
        let checked_streams = {
            let mut state = self.locked()?;
            self.checked_completion(&state, &document)
                .and_then(
                    |directory| match state.writers.remove(&document.invocation_id) {
                        Some(streams) => Ok((directory, streams)),
                        None => Err(ComputeLogError::rejected(
                            ComputeLogErrorKind::State,
                            "capture is not active in this storage instance",
                        )),
                    },
                )
        };
        let (directory, streams) = checked(py, checked_streams)?;
        checked(py, complete_capture(&directory, &document, streams))
    }

    /// Delete a complete inactive capture without following symlinks.
    #[pyo3(signature = (*, invocation_id))]
    fn delete(&self, py: Python<'_>, invocation_id: &Bound<'_, PyAny>) -> PyResult<()> {
        let invocation_id = capture_id(invocation_id)?;
        let state = self.locked()?;
        let active = state.writers.contains_key(&invocation_id);
        let result = delete_capture(&self.root, &invocation_id, active);
        drop(state);
        checked(py, result)
    }

    /// Retain the newest complete captures while never deleting incomplete captures.
    #[pyo3(signature = (retain_count = None))]
    fn prune<'py>(
        &self,
        py: Python<'py>,
        retain_count: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let retain: Option<usize> = match retain_count {
            None => self
                .retention_count
                .map(|count| usize::try_from(count).unwrap_or(0)),
            Some(value) => Some(usize::try_from(exact_count(value, -1)).map_err(|_| {
                rejection(
                    py,
                    ComputeLogErrorKind::InvalidLimit,
                    "retain_count must be nonnegative or None",
                )
            })?),
        };
        let active: HashSet<String> = self.locked()?.writers.keys().cloned().collect();
        let root = self.root.clone();
        let outcome = checked(
            py,
            py.detach(|| prune_captures(&root, retain, &|id: &str| active.contains(id))),
        )?;
        call_with(
            &host_entry(py, ExecutorHost::PruneResult)?,
            &[
                (
                    "deleted_invocation_ids",
                    PyTuple::new(py, &outcome.deleted_invocation_ids)?.into_any(),
                ),
                (
                    "retained_complete_count",
                    outcome.retained_complete_count.into_bound_py_any(py)?,
                ),
                (
                    "retained_incomplete_count",
                    outcome.retained_incomplete_count.into_bound_py_any(py)?,
                ),
            ],
        )
    }

    /// Inventory complete and abandoned captures without synthesizing lifecycle state.
    fn inventory<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let entries = checked(py, capture_inventory(&self.root))?;
        let mut items: Vec<Bound<'py, PyAny>> = Vec::new();
        for entry in &entries {
            items.push(inventory_item(py, entry)?);
        }
        let complete_count = entries.iter().filter(|entry| entry.is_complete).count();
        call_with(
            &host_entry(py, ExecutorHost::CaptureInventory)?,
            &[
                ("captures", PyTuple::new(py, items)?.into_any()),
                ("complete_count", complete_count.into_bound_py_any(py)?),
                (
                    "incomplete_count",
                    (entries.len() - complete_count).into_bound_py_any(py)?,
                ),
            ],
        )
    }

    /// Close active writers, deliberately leaving their captures incomplete.
    fn close(&self) -> PyResult<()> {
        let mut state = self.locked()?;
        state.closed = true;
        state.writers.clear();
        Ok(())
    }

    /// Dispose process-local resources without deleting retained logs.
    fn dispose(&self) -> PyResult<()> {
        self.close()
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    #[pyo3(signature = (*_exc_info))]
    fn __exit__(&self, _exc_info: &Bound<'_, PyTuple>) -> PyResult<Option<Py<PyAny>>> {
        self.close()?;
        Ok(None)
    }
}
