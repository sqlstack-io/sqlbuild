//! Write, clean and publish compiled artifacts natively, raising the errors Python raised.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use pyo3::exceptions::PyOSError;
use pyo3::prelude::{PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyBytes;
use sqlbuild_cache::artifacts::errors::ArtifactError;
use sqlbuild_cache::artifacts::main::write_artifacts::write_artifacts;
use sqlbuild_cache::artifacts::models::WrittenArtifacts;

pub(crate) type PublishedRow = (String, Vec<(String, String)>);

/// One write group's `(path, contents)` artifacts and whether unchanged files are kept.
pub(crate) type ArtifactGroup = (Vec<(PathBuf, Vec<u8>)>, bool);

pub(crate) const WRITER_THREAD: &str = "sqlbuild-artifact-writer";
const WRITER_POLL: Duration = Duration::from_millis(50);

/// Write each group in order until one fails or a stop is requested between groups.
pub(crate) fn write_groups(
    groups: &Receiver<ArtifactGroup>,
    stop: &AtomicBool,
) -> (Result<(), ArtifactError>, Duration) {
    let mut elapsed: Duration = Duration::ZERO;
    for (files, check_existing) in groups {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let started: Instant = Instant::now();
        let written: Result<WrittenArtifacts, ArtifactError> =
            write_artifacts(&files, check_existing);
        elapsed += started.elapsed();
        if let Err(error) = written {
            return (Err(error), elapsed);
        }
    }
    (Ok(()), elapsed)
}

/// Whether the writer thread has returned, with a condition variable to wait on it.
#[derive(Debug, Default)]
pub(crate) struct WriterDone {
    pub(crate) finished: Mutex<bool>,
    pub(crate) changed: Condvar,
}

/// Marks the writer done when the writer thread returns or unwinds.
pub(crate) struct DoneMarker(pub(crate) Arc<WriterDone>);

impl Drop for DoneMarker {
    fn drop(&mut self) {
        *self
            .0
            .finished
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = true;
        self.0.changed.notify_all();
    }
}

impl WriterDone {
    /// Whether the writer returned within `timeout`.
    pub(crate) fn finished_within(&self, timeout: Duration) -> bool {
        let finished = self.finished.lock().unwrap_or_else(PoisonError::into_inner);
        let (finished, _) = self
            .changed
            .wait_timeout_while(finished, timeout, |finished| !*finished)
            .unwrap_or_else(PoisonError::into_inner);
        *finished
    }
}

/// Wait without the GIL until the writer returns, checking for signals between waits.
pub(crate) fn wait_for_writer(py: Python<'_>, finished: &WriterDone) -> PyResult<()> {
    while !py.detach(|| finished.finished_within(WRITER_POLL)) {
        py.check_signals()?;
    }
    Ok(())
}

pub(crate) fn lossy(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The exception Python's writer raises for the same failure.
pub(crate) fn artifact_error(py: Python<'_>, error: ArtifactError) -> PyErr {
    match error {
        ArtifactError::Io { path, error } => match error.raw_os_error() {
            Some(code) => PyOSError::new_err((code, strerror(py, code), lossy(&path))),
            None => PyOSError::new_err(error.to_string()),
        },
        ArtifactError::Move {
            source,
            path,
            error,
        } => match error.raw_os_error() {
            Some(code) => PyOSError::new_err((
                code,
                strerror(py, code),
                lossy(&source),
                py.None(),
                lossy(&path),
            )),
            None => PyOSError::new_err(error.to_string()),
        },
        ArtifactError::ExistingNotUtf8 { path } => existing_decode_error(py, &path),
    }
}

fn strerror(py: Python<'_>, code: i32) -> String {
    py.import("os")
        .and_then(|os| os.call_method1("strerror", (code,)))
        .and_then(|text| text.extract::<String>())
        .unwrap_or_default()
}

/// Decode the existing file in Python so the error is exactly the one `bytes.decode` raises.
fn existing_decode_error(py: Python<'_>, path: &PathBuf) -> PyErr {
    let bytes: Vec<u8> = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => return PyOSError::new_err(error.to_string()),
    };
    match PyBytes::new(py, &bytes).call_method1("decode", ("utf-8",)) {
        Err(error) => error,
        Ok(_) => PyOSError::new_err(format!("compiled artifact '{}' changed", path.display())),
    }
}
