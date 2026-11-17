//! Compiled artifacts queued for one native write, keeping unchanged files when asked.

use std::path::PathBuf;

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_host::compile_context::main::add_compile_timing::add_compile_timing;
use sqlbuild_host::compile_context::main::record_compile_timing::record_compile_timing;

use crate::bindings::_helpers::compile_command::reuse_recording::ArtifactRecorder;
use crate::bindings::_helpers::compile_command::reuse_recording::artifact_written;
use crate::bindings::_helpers::compile_command::reuse_recording::artifact_written_at;
use crate::bindings::_helpers::compile_outputs::artifacts::BackgroundArtifactWriter;
use crate::bindings::_helpers::compile_outputs::artifacts::write_compiled_artifacts;

const PHYSICAL_WRITE_PHASE: &str = "physical_write_ms";

/// The artifacts of one write group, written together by `flush`.
pub(crate) struct ArtifactBatch<'a> {
    check_existing: bool,
    recorder: ArtifactRecorder<'a>,
    files: Vec<(PathBuf, Vec<u8>)>,
    /// Earlier groups, still being written in the background.
    writer: Option<BackgroundArtifactWriter>,
}

/// `contents` with platform line endings.
fn platform_lines(contents: Vec<u8>) -> Vec<u8> {
    if cfg!(windows) {
        contents
            .iter()
            .flat_map(|byte| {
                if *byte == b'\n' {
                    vec![b'\r', b'\n']
                } else {
                    vec![*byte]
                }
            })
            .collect()
    } else {
        contents
    }
}

/// `sql` with trailing whitespace replaced by one newline.
fn sql_artifact(sql: &str) -> Vec<u8> {
    let trimmed: &str =
        sql.trim_end_matches(sqlbuild_core::text::main::python_isspace::python_isspace);
    format!("{trimmed}\n").into_bytes()
}

impl<'a> ArtifactBatch<'a> {
    pub(crate) fn new(check_existing: bool, recorder: ArtifactRecorder<'a>) -> Self {
        Self {
            check_existing,
            recorder,
            files: Vec::new(),
            writer: None,
        }
    }

    /// Queue one artifact with platform line endings, noting its bytes for compile reuse.
    pub(crate) fn queue(&mut self, path: &Bound<'_, PyAny>, contents: Vec<u8>) -> PyResult<()> {
        let contents: Vec<u8> = platform_lines(contents);
        artifact_written(self.recorder, path, &contents)?;
        self.files.push((path.extract()?, contents));
        Ok(())
    }

    /// Queue `sql` with trailing whitespace replaced by one newline.
    pub(crate) fn queue_sql(&mut self, path: &Bound<'_, PyAny>, sql: &str) -> PyResult<()> {
        self.queue(path, sql_artifact(sql))
    }

    /// `queue_sql` at a native path.
    pub(crate) fn queue_sql_at(
        &mut self,
        py: Python<'_>,
        path: PathBuf,
        sql: &str,
    ) -> PyResult<()> {
        let contents: Vec<u8> = platform_lines(sql_artifact(sql));
        artifact_written_at(py, self.recorder, &path, &contents)?;
        self.files.push((path, contents));
        Ok(())
    }

    /// Write every queued artifact natively, after any background group, and empty the queue.
    pub(crate) fn flush(&mut self, py: Python<'_>) -> PyResult<()> {
        self.finish_pending(py)?;
        if self.files.is_empty() {
            return Ok(());
        }
        let files: Vec<(PathBuf, Vec<u8>)> = std::mem::take(&mut self.files);
        record_compile_timing(PHYSICAL_WRITE_PHASE, || {
            write_compiled_artifacts(py, files, self.check_existing).map(|_| ())
        })
    }

    /// Write the queued artifacts in the background, after every earlier background group.
    pub(crate) fn flush_in_background(&mut self, py: Python<'_>) -> PyResult<()> {
        if self.files.is_empty() {
            return Ok(());
        }
        if self.writer.is_none() {
            self.writer = BackgroundArtifactWriter::start();
        }
        let Some(writer) = &self.writer else {
            return self.flush(py);
        };
        writer.send(std::mem::take(&mut self.files), self.check_existing);
        Ok(())
    }

    /// `result`, except that a background group's earlier write error wins and interrupts stop it.
    pub(crate) fn settle<T>(&mut self, py: Python<'_>, result: PyResult<T>) -> PyResult<T> {
        let error: PyErr = match result {
            Ok(value) => return Ok(value),
            Err(error) => error,
        };
        let Some(writer) = self.writer.take() else {
            return Err(error);
        };
        if !error.is_instance_of::<PyException>(py) {
            writer.stop(py);
            return Err(error);
        }
        writer.finish(py)?;
        Err(error)
    }

    /// Wait for the background groups, raising the first write error.
    fn finish_pending(&mut self, py: Python<'_>) -> PyResult<()> {
        let Some(writer) = self.writer.take() else {
            return Ok(());
        };
        let elapsed = writer.finish(py)?;
        add_compile_timing(
            PHYSICAL_WRITE_PHASE,
            u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX),
        );
        Ok(())
    }
}

impl Drop for ArtifactBatch<'_> {
    fn drop(&mut self) {
        if let Some(writer) = self.writer.take() {
            Python::attach(|py| writer.stop(py));
        }
    }
}
