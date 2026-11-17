//! Public records of compile command.

use crate::compile_command::_helpers::artifacts::artifact_batch::PHYSICAL_WRITE_PHASE;
use crate::compile_command::_helpers::artifacts::artifact_batch::platform_lines;
use crate::compile_command::_helpers::artifacts::artifact_batch::sql_artifact;
use crate::compile_command::_helpers::invocation::reuse_recording::PrintedOutput;
use crate::compile_command::_helpers::invocation::reuse_recording::artifact_written;
use crate::compile_command::_helpers::invocation::reuse_recording::artifact_written_at;
use crate::compile_command::constants::REUSE_ATTEMPT_FIELDS;
use crate::compile_command::main::invocation::print_to::print_to;
use crate::compile_command::types::ArtifactRecorder;
use crate::compile_outputs::main::write_compiled_artifacts::write_compiled_artifacts;
use crate::compile_outputs::models::BackgroundArtifactWriter;
use crate::compile_outputs::types::ArtifactRow;
use pyo3::IntoPyObject;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyException;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use sqlbuild_cache::project_reuse::models::RecordedArtifact;
use sqlbuild_host::compile_context::main::add_compile_timing::add_compile_timing;
use sqlbuild_host::compile_context::main::record_compile_timing::record_compile_timing;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::PoisonError;

/// The artifacts of one write group, written together by `flush`.
#[derive(Debug)]
pub struct ArtifactBatch<'a> {
    pub(crate) check_existing: bool,
    pub(crate) recorder: ArtifactRecorder<'a>,
    pub(crate) files: Vec<(PathBuf, Vec<u8>)>,
    /// Earlier groups, still being written in the background.
    pub(crate) writer: Option<BackgroundArtifactWriter>,
}

impl<'a> ArtifactBatch<'a> {
    pub fn new(check_existing: bool, recorder: ArtifactRecorder<'a>) -> Self {
        Self {
            check_existing,
            recorder,
            files: Vec::new(),
            writer: None,
        }
    }

    /// Queue one artifact with platform line endings, noting its bytes for compile reuse.
    pub fn queue(&mut self, path: &Bound<'_, PyAny>, contents: Vec<u8>) -> PyResult<()> {
        let contents: Vec<u8> = platform_lines(contents);
        artifact_written(self.recorder, path, &contents)?;
        self.files.push((path.extract()?, contents));
        Ok(())
    }

    /// Queue `sql` with trailing whitespace replaced by one newline.
    pub fn queue_sql(&mut self, path: &Bound<'_, PyAny>, sql: &str) -> PyResult<()> {
        self.queue(path, sql_artifact(sql))
    }

    /// `queue_sql` at a native path.
    pub fn queue_sql_at(&mut self, py: Python<'_>, path: PathBuf, sql: &str) -> PyResult<()> {
        let contents: Vec<u8> = platform_lines(sql_artifact(sql));
        artifact_written_at(py, self.recorder, &path, &contents)?;
        self.files.push((path, contents));
        Ok(())
    }

    /// Write every queued artifact natively, after any background group, and empty the queue.
    pub fn flush(&mut self, py: Python<'_>) -> PyResult<()> {
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
    pub fn flush_in_background(&mut self, py: Python<'_>) -> PyResult<()> {
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
    pub fn settle<T>(&mut self, py: Python<'_>, result: PyResult<T>) -> PyResult<T> {
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
    pub(crate) fn finish_pending(&mut self, py: Python<'_>) -> PyResult<()> {
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

/// Result of checking for a reusable compile, carried into a full compile on a miss.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub struct CompileReuseAttempt {
    /// `hit`, `miss` or `bypass`.
    pub outcome: Py<PyAny>,
    pub project_dir: Py<PyAny>,
    pub started: Py<PyAny>,
    pub check_ms: Py<PyAny>,
    pub exit_code: Py<PyAny>,
    pub entry_path: Py<PyAny>,
    /// The natively checked attempt, recorded natively after a miss.
    pub native: Py<PyAny>,
    pub selected_target: Py<PyAny>,
    pub json_output: Py<PyAny>,
}

#[pymethods]
impl CompileReuseAttempt {
    #[new]
    #[pyo3(signature = (**options))]
    pub(crate) fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let fields: KeywordArguments<'_> =
            KeywordArguments::read(py, "CompileReuseAttempt", options, REUSE_ATTEMPT_FIELDS)?;
        let field = |name: &str| -> PyResult<Py<PyAny>> { Ok(fields.value(name)?.unbind()) };
        Ok(Self {
            outcome: field("outcome")?,
            project_dir: field("project_dir")?,
            started: field("started")?,
            check_ms: if fields.passed("check_ms")? {
                field("check_ms")?
            } else {
                0_i64.into_py_any(py)?
            },
            exit_code: field("exit_code")?,
            entry_path: field("entry_path")?,
            native: field("native")?,
            selected_target: field("selected_target")?,
            json_output: fields.value_or_false("json_output")?.unbind(),
        })
    }
}

/// Artifacts one compile wrote or kept, by absolute path, following staged moves.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct CompileArtifactWrites {
    pub(crate) artifacts: Mutex<HashMap<String, RecordedArtifact>>,
}

impl CompileArtifactWrites {
    pub(crate) fn locked(&self) -> std::sync::MutexGuard<'_, HashMap<String, RecordedArtifact>> {
        self.artifacts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn record(&self, path: String, artifact: RecordedArtifact) {
        let _ = self.locked().insert(path, artifact);
    }

    pub(crate) fn move_one(&self, source: &str, destination: String) {
        let mut artifacts = self.locked();
        if let Some(artifact) = artifacts.remove(source) {
            let _ = artifacts.insert(destination, artifact);
        }
    }

    pub(crate) fn move_tree(&self, source: &str, destination: &str) {
        let prefix: String = format!("{source}{}", std::path::MAIN_SEPARATOR);
        let mut artifacts = self.locked();
        let moved: Vec<(String, RecordedArtifact)> = artifacts
            .iter()
            .filter(|(path, _)| path.starts_with(&prefix))
            .map(|(path, artifact)| {
                (
                    format!("{destination}{}", &path[source.len()..]),
                    artifact.clone(),
                )
            })
            .collect();
        artifacts.retain(|path, _| !path.starts_with(&prefix));
        artifacts.extend(moved);
    }
}

impl CompileArtifactWrites {
    /// Every recorded artifact's absolute path, with its digest when this compile wrote it.
    pub fn written_digests(&self) -> HashMap<String, Option<String>> {
        self.locked()
            .iter()
            .map(|(path, artifact)| {
                let digest: Option<String> = match artifact {
                    RecordedArtifact::Written { digest } => Some(digest.clone()),
                    RecordedArtifact::Kept { .. } => None,
                };
                (path.clone(), digest)
            })
            .collect()
    }

    /// The Python-facing artifact rows, built natively for recording or the public view.
    pub(crate) fn artifact_rows(&self) -> HashMap<String, ArtifactRow> {
        self.locked()
            .iter()
            .map(|(path, artifact)| {
                let row = match artifact {
                    RecordedArtifact::Written { digest } => (Some(digest.clone()), None, None),
                    RecordedArtifact::Kept { size, mtime_ns } => {
                        (None, Some(*size), Some(*mtime_ns))
                    }
                };
                (path.clone(), row)
            })
            .collect()
    }
}

#[pymethods]
impl CompileArtifactWrites {
    /// Every recorded artifact by absolute path, as `(digest, size, mtime_ns)`.
    #[getter]
    pub fn artifacts<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        self.artifact_rows().into_pyobject(py)
    }
}

/// Compile output printed at once (notes) or after storing (the report), keeping the text.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct RecordedCompileOutput {
    pub(crate) output: Mutex<PrintedOutput>,
}

impl RecordedCompileOutput {
    pub(crate) fn new() -> Self {
        Self {
            output: Mutex::new((Vec::new(), None)),
        }
    }

    pub(crate) fn locked(&self) -> std::sync::MutexGuard<'_, PrintedOutput> {
        self.output.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Output already printed or recorded elsewhere: its stderr lines and its report.
    pub fn recorded(py: Python<'_>, stderr_lines: &[String], stdout: &str) -> Self {
        let lines: Vec<Py<PyString>> = stderr_lines
            .iter()
            .map(|line| PyString::new(py, line).unbind())
            .collect();
        Self {
            output: Mutex::new((lines, Some(PyString::new(py, stdout).unbind()))),
        }
    }

    /// The recorded stderr lines and report, as text.
    pub fn texts(&self, py: Python<'_>) -> PyResult<(Vec<String>, Option<String>)> {
        let output = self.locked();
        let lines: Vec<String> = output
            .0
            .iter()
            .map(|line| Ok(line.bind(py).to_str()?.to_owned()))
            .collect::<PyResult<_>>()?;
        let stdout: Option<String> = match &output.1 {
            Some(text) => Some(text.bind(py).to_str()?.to_owned()),
            None => None,
        };
        Ok((lines, stdout))
    }
}

#[pymethods]
impl RecordedCompileOutput {
    /// Print one stderr result line, such as a note, and record it; a delta only records it.
    pub fn print_stderr_line(&self, line: Bound<'_, PyString>) -> PyResult<()> {
        if sqlbuild_python_base::compile_context::main::delta::injection::injection().is_none() {
            print_to(line.py(), "stderr", &line)?;
        }
        self.locked().0.push(line.unbind());
        Ok(())
    }

    /// Record the compile report; it is printed once the compile has been stored.
    pub fn record_stdout(&self, text: &Bound<'_, PyString>) -> PyResult<()> {
        let recorded: Bound<'_, PyString> = text.add("\n")?.downcast_into()?;
        self.locked().1 = Some(recorded.unbind());
        Ok(())
    }

    /// Print the recorded compile report, if any, exactly as it is stored.
    pub fn print_stdout(&self, py: Python<'_>) -> PyResult<()> {
        let Some(text) = self.locked().1.as_ref().map(|text| text.clone_ref(py)) else {
            return Ok(());
        };
        let stdout: Bound<'_, PyAny> = py.import("sys")?.getattr("stdout")?;
        let _ = stdout.call_method1("write", (text,))?;
        let _ = stdout.call_method0("flush")?;
        Ok(())
    }

    #[getter]
    pub fn stderr_lines(&self, py: Python<'_>) -> Vec<Py<PyString>> {
        self.locked()
            .0
            .iter()
            .map(|line| line.clone_ref(py))
            .collect()
    }

    #[getter]
    pub fn stdout(&self, py: Python<'_>) -> Option<Py<PyString>> {
        self.locked().1.as_ref().map(|text| text.clone_ref(py))
    }
}

/// What `write_reusable_compile` records about one full compile.
#[derive(Debug)]
pub struct CompileFacts<'py> {
    pub(crate) native: Bound<'py, PyAny>,
    pub(crate) output: Bound<'py, PyAny>,
    pub(crate) exit_code: Bound<'py, PyAny>,
    pub(crate) input_reads: Bound<'py, PyAny>,
    pub(crate) artifact_writes: Bound<'py, PyAny>,
    pub(crate) compile_cache_enabled: bool,
    pub(crate) artifacts_written: bool,
    pub(crate) dag_artifact_path: Bound<'py, PyAny>,
    pub(crate) json_output: Bound<'py, PyAny>,
}

/// SQL tests planned ahead of the write phase, and where the compile notes its artifact writes.
#[derive(Debug, Clone, Copy, Default)]
pub struct Prepared<'a, 'py> {
    /// `planned_tests()` hands over the background plan, when one was started.
    pub planned_tests: Option<&'a Bound<'py, PyAny>>,
    pub recorder: ArtifactRecorder<'a>,
}
