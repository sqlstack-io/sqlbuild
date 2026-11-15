//! Write, clean and publish compiled artifacts natively, raising the errors Python raised.

use std::collections::HashSet;
use std::panic::resume_unwind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use pyo3::exceptions::PyOSError;
use pyo3::prelude::{PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyBytes;
use sqlbuild_cache::artifacts::errors::ArtifactError;
use sqlbuild_cache::artifacts::main::publish_staged::publish_staged;
use sqlbuild_cache::artifacts::main::remove_stale::remove_stale;
use sqlbuild_cache::artifacts::main::remove_stale_unless_unchanged::remove_stale_unless_unchanged;
use sqlbuild_cache::artifacts::main::staged_files::staged_files;
use sqlbuild_cache::artifacts::main::write_artifacts::write_artifacts;
use sqlbuild_cache::artifacts::models::{Publication, WrittenArtifacts};

use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;

type PublishedRow = (String, Vec<(String, String)>);

/// One write group's `(path, contents)` artifacts and whether unchanged files are kept.
type ArtifactGroup = (Vec<(PathBuf, Vec<u8>)>, bool);

const WRITER_THREAD: &str = "sqlbuild-artifact-writer";
const WRITER_POLL: Duration = Duration::from_millis(50);

/// Write each `(path, contents)` artifact; returns how many were written and left unchanged.
pub(crate) fn write_compiled_artifacts(
    py: Python<'_>,
    files: Vec<(PathBuf, Vec<u8>)>,
    check_existing: bool,
) -> PyResult<(usize, usize)> {
    compiler_guard(|| {
        let outcome: Result<WrittenArtifacts, ArtifactError> =
            py.detach(|| write_artifacts(&files, check_existing));
        let written: WrittenArtifacts = outcome.map_err(|error| artifact_error(py, error))?;
        Ok((written.written, written.unchanged))
    })
}

/// Artifact groups written one after another on their own thread, stopping at the first error.
pub(crate) struct BackgroundArtifactWriter {
    groups: Sender<ArtifactGroup>,
    /// Set to skip every group not yet started; the group being written still completes.
    stop: Arc<AtomicBool>,
    /// Marked once the writer returns, so a wait can check for signals meanwhile.
    finished: Arc<WriterDone>,
    handle: JoinHandle<(Result<(), ArtifactError>, Duration)>,
}

/// Write each group in order until one fails or a stop is requested between groups.
fn write_groups(
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

impl BackgroundArtifactWriter {
    /// Start the writer thread, or `None` when no thread could start.
    pub(crate) fn start() -> Option<Self> {
        let (groups, received) = channel();
        let finished: Arc<WriterDone> = Arc::new(WriterDone::default());
        let marker: DoneMarker = DoneMarker(Arc::clone(&finished));
        let stop: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let stopping: Arc<AtomicBool> = Arc::clone(&stop);
        let spawned = std::thread::Builder::new()
            .name(WRITER_THREAD.to_owned())
            .spawn(move || {
                let _marker: DoneMarker = marker;
                write_groups(&received, &stopping)
            });
        match spawned {
            Ok(handle) => Some(Self {
                groups,
                stop,
                finished,
                handle,
            }),
            Err(_) => None,
        }
    }

    /// Queue one group behind the groups already sent; a writer that stopped drops it.
    pub(crate) fn send(&self, files: Vec<(PathBuf, Vec<u8>)>, check_existing: bool) {
        let _ = self.groups.send((files, check_existing));
    }

    /// Wait without the GIL for every sent group; a signal stops it after the current group.
    pub(crate) fn finish(self, py: Python<'_>) -> PyResult<Duration> {
        let Self {
            groups,
            stop,
            finished,
            handle,
        } = self;
        drop(groups);
        if let Err(interrupt) = wait_for_writer(py, &finished) {
            stop.store(true, Ordering::SeqCst);
            let _ = py.detach(|| handle.join());
            return Err(interrupt);
        }
        compiler_guard(|| {
            let joined = py.detach(|| handle.join());
            let (outcome, elapsed) = joined.unwrap_or_else(|payload| resume_unwind(payload));
            outcome.map_err(|error| artifact_error(py, error))?;
            Ok(elapsed)
        })
    }

    /// Stop after the group being written and wait for it without the GIL, ignoring the outcome.
    pub(crate) fn stop(self, py: Python<'_>) {
        let Self {
            groups,
            stop,
            finished,
            handle,
        } = self;
        stop.store(true, Ordering::SeqCst);
        drop(groups);
        drop(finished);
        let _ = py.detach(|| handle.join());
    }
}

/// Whether the writer thread has returned, with a condition variable to wait on it.
#[derive(Default)]
struct WriterDone {
    finished: Mutex<bool>,
    changed: Condvar,
}

/// Marks the writer done when the writer thread returns or unwinds.
struct DoneMarker(Arc<WriterDone>);

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
    fn finished_within(&self, timeout: Duration) -> bool {
        let finished = self.finished.lock().unwrap_or_else(PoisonError::into_inner);
        let (finished, _) = self
            .changed
            .wait_timeout_while(finished, timeout, |finished| !*finished)
            .unwrap_or_else(PoisonError::into_inner);
        *finished
    }
}

/// Wait without the GIL until the writer returns, checking for signals between waits.
fn wait_for_writer(py: Python<'_>, finished: &WriterDone) -> PyResult<()> {
    while !py.detach(|| finished.finished_within(WRITER_POLL)) {
        py.check_signals()?;
    }
    Ok(())
}

/// Delete unmanaged compiled files and empty directories; returns how many files were removed.
pub(crate) fn remove_stale_artifacts(
    py: Python<'_>,
    compiled_dir: PathBuf,
    managed: Vec<PathBuf>,
    stamp: Option<(PathBuf, i64)>,
) -> PyResult<usize> {
    compiler_guard(|| {
        let managed: HashSet<PathBuf> = managed.into_iter().collect();
        py.detach(|| match &stamp {
            Some((stamp_path, racy_window_ns)) => {
                remove_stale_unless_unchanged(&compiled_dir, &managed, stamp_path, *racy_window_ns)
            }
            None => remove_stale(&compiled_dir, &managed),
        })
        .map_err(|error| artifact_error(py, error))
    })
}

/// Publish staged artifacts: `("changed", [])`, `("tree", [])` or `("files", [(source, path)])`.
pub(crate) fn publish_staged_artifacts(
    py: Python<'_>,
    staged_dir: PathBuf,
    compiled_dir: PathBuf,
    expected: Vec<PathBuf>,
) -> PyResult<PublishedRow> {
    compiler_guard(|| {
        let expected: HashSet<PathBuf> = expected.into_iter().collect();
        let publication: Publication = py
            .detach(|| publish_staged(&staged_dir, &compiled_dir, &expected))
            .map_err(|error| artifact_error(py, error))?;
        Ok(match publication {
            Publication::StagedChanged => ("changed".to_owned(), Vec::new()),
            Publication::MovedTree => ("tree".to_owned(), Vec::new()),
            Publication::Files(files) => (
                "files".to_owned(),
                files
                    .into_iter()
                    .map(|(source, path)| (lossy(&source), lossy(&path)))
                    .collect(),
            ),
        })
    })
}

/// Every staged artifact's path relative to `staged_dir`.
pub(crate) fn staged_artifact_files(py: Python<'_>, staged_dir: PathBuf) -> PyResult<Vec<String>> {
    compiler_guard(|| {
        let files: Vec<PathBuf> = py.detach(|| staged_files(&staged_dir));
        Ok(files.iter().map(|path| lossy(path)).collect())
    })
}

fn lossy(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The exception Python's writer raises for the same failure.
fn artifact_error(py: Python<'_>, error: ArtifactError) -> PyErr {
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
