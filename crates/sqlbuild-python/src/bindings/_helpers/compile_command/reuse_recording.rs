//! What one full compile printed and left in each artifact, and storing it for compile reuse.

use sqlbuild_cache::project_reuse::types::ImageSections;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use pyo3::exceptions::{PyException, PyOSError};
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyRef, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyString, PyStringMethods};
use pyo3::{IntoPyObject, IntoPyObjectExt, pyclass, pyfunction, pymethods, wrap_pyfunction};

use sqlbuild_cache::project_reuse::models::RecordedArtifact;
use sqlbuild_core::posix_path::main::posix_abspath::posix_abspath;
use sqlbuild_host::entries::models::{FacadeHost, UserCodeHost};

use crate::bindings::_helpers::boundary::python_calls::{
    KeywordArguments, call_with, imported, keywords,
};
use crate::bindings::_helpers::compile_command::constants::{
    BYTES_PER_MEBIBYTE, REUSE_BYPASS_TIMING, REUSE_CHECK_TIMING, REUSE_HIT_TIMING,
    REUSE_LOGGER_NAME, REUSE_MISS_TIMING, REUSE_STORE_DONE_PREFIX, REUSE_STORE_NOTICE_BYTES,
    REUSE_STORE_NOTICE_PATHS, REUSE_STORE_SKIPPED_PREFIX,
};
use crate::bindings::_helpers::compile_command::reuse_attempt::{reuse_rules, write_launch_record};
use crate::bindings::_helpers::compile_outputs::reuse::{
    ArtifactRow, CompileRecordPy, NativeReuseAttempt, SettingsRow, artifact_digest,
};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::fields::field as native_field;

const MISS_OUTCOME: &str = "miss";
const HIT_OUTCOME: &str = "hit";
const BYPASS_OUTCOME: &str = "bypass";
const STORED: &str = "stored";
const SKIPPED: &str = "skipped";

/// Artifacts one compile wrote or kept, by absolute path, following staged moves.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct CompileArtifactWrites {
    artifacts: Mutex<HashMap<String, RecordedArtifact>>,
}

impl CompileArtifactWrites {
    fn locked(&self) -> std::sync::MutexGuard<'_, HashMap<String, RecordedArtifact>> {
        self.artifacts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn record(&self, path: String, artifact: RecordedArtifact) {
        let _ = self.locked().insert(path, artifact);
    }

    fn move_one(&self, source: &str, destination: String) {
        let mut artifacts = self.locked();
        if let Some(artifact) = artifacts.remove(source) {
            let _ = artifacts.insert(destination, artifact);
        }
    }

    fn move_tree(&self, source: &str, destination: &str) {
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
    pub(crate) fn written_digests(&self) -> HashMap<String, Option<String>> {
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
    fn artifact_rows(&self) -> HashMap<String, ArtifactRow> {
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
    pub(crate) fn artifacts<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        self.artifact_rows().into_pyobject(py)
    }
}

/// Where a compile notes the artifacts it writes; `None` outside a recorded compile.
pub(crate) type ArtifactRecorder<'a> = Option<&'a Py<CompileArtifactWrites>>;

/// `os.path.abspath(path)`, computed natively on POSIX for UTF-8 paths and working directories.
fn absolute(path: &Bound<'_, PyAny>) -> PyResult<String> {
    absolute_path(path.py(), &path.extract::<PathBuf>()?)
}

/// `os.path.abspath(path)` of a native path.
fn absolute_path(py: Python<'_>, path: &Path) -> PyResult<String> {
    if cfg!(unix)
        && let Some(found) = native_absolute(path)
    {
        return Ok(found);
    }
    imported(py, "os.path", "abspath")?
        .call1((path,))?
        .extract()
}

fn native_absolute(path: &Path) -> Option<String> {
    let Ok(cwd) = std::env::current_dir() else {
        return None;
    };
    Some(posix_abspath(cwd.to_str()?, path.to_str()?))
}

/// A new, empty recorder of one compile's artifact writes.
pub(crate) fn new_artifact_recorder(py: Python<'_>) -> PyResult<Py<CompileArtifactWrites>> {
    Py::new(
        py,
        CompileArtifactWrites {
            artifacts: Mutex::new(HashMap::new()),
        },
    )
}

/// Note the exact bytes an artifact holds after a write or an unchanged-file check.
pub(crate) fn artifact_written(
    recorder: ArtifactRecorder<'_>,
    path: &Bound<'_, PyAny>,
    contents: &[u8],
) -> PyResult<()> {
    let Some(recorder) = recorder else {
        return Ok(());
    };
    recorder.get().record(
        absolute(path)?,
        RecordedArtifact::Written {
            digest: artifact_digest(contents),
        },
    );
    Ok(())
}

/// `artifact_written` for a native path.
pub(crate) fn artifact_written_at(
    py: Python<'_>,
    recorder: ArtifactRecorder<'_>,
    path: &Path,
    contents: &[u8],
) -> PyResult<()> {
    let Some(recorder) = recorder else {
        return Ok(());
    };
    recorder.get().record(
        absolute_path(py, path)?,
        RecordedArtifact::Written {
            digest: artifact_digest(contents),
        },
    );
    Ok(())
}

/// Note an earlier artifact kept as-is after its size and mtime were validated.
pub(crate) fn artifact_kept(
    py: Python<'_>,
    recorder: ArtifactRecorder<'_>,
    path: &Path,
    (size, mtime_ns): (u64, i64),
) -> PyResult<()> {
    let Some(recorder) = recorder else {
        return Ok(());
    };
    recorder.get().record(
        absolute_path(py, path)?,
        RecordedArtifact::Kept { size, mtime_ns },
    );
    Ok(())
}

/// Note that one staged artifact was published at another path.
pub(crate) fn artifact_moved(
    recorder: ArtifactRecorder<'_>,
    source: &Bound<'_, PyAny>,
    destination: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let Some(recorder) = recorder else {
        return Ok(());
    };
    recorder
        .get()
        .move_one(&absolute(source)?, absolute(destination)?);
    Ok(())
}

/// Note that a staged artifact directory was published at another directory.
pub(crate) fn artifact_tree_moved(
    recorder: ArtifactRecorder<'_>,
    source: &Bound<'_, PyAny>,
    destination: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let Some(recorder) = recorder else {
        return Ok(());
    };
    recorder
        .get()
        .move_tree(&absolute(source)?, &absolute(destination)?);
    Ok(())
}

/// The stderr lines and stdout report one compile printed, kept as the Python text it printed.
type PrintedOutput = (Vec<Py<PyString>>, Option<Py<PyString>>);

/// Compile output printed at once (notes) or after storing (the report), keeping the text.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct RecordedCompileOutput {
    output: Mutex<PrintedOutput>,
}

impl RecordedCompileOutput {
    pub(crate) fn new() -> Self {
        Self {
            output: Mutex::new((Vec::new(), None)),
        }
    }

    fn locked(&self) -> std::sync::MutexGuard<'_, PrintedOutput> {
        self.output.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Output already printed or recorded elsewhere: its stderr lines and its report.
    pub(crate) fn recorded(py: Python<'_>, stderr_lines: &[String], stdout: &str) -> Self {
        let lines: Vec<Py<PyString>> = stderr_lines
            .iter()
            .map(|line| PyString::new(py, line).unbind())
            .collect();
        Self {
            output: Mutex::new((lines, Some(PyString::new(py, stdout).unbind()))),
        }
    }

    /// The recorded stderr lines and report, as text.
    pub(crate) fn texts(&self, py: Python<'_>) -> PyResult<(Vec<String>, Option<String>)> {
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

/// `print(*values, file=sys.<stream>)`.
pub(crate) fn print_to<'py>(
    py: Python<'py>,
    stream: &str,
    text: impl pyo3::IntoPyObject<'py>,
) -> PyResult<()> {
    let keywords: Bound<'_, PyDict> =
        keywords(py, &[("file", py.import("sys")?.getattr(stream)?)])?;
    let _ = py
        .import("builtins")?
        .getattr("print")?
        .call((text,), Some(&keywords))?;
    Ok(())
}

#[pymethods]
impl RecordedCompileOutput {
    /// Print one stderr result line, such as a note, and record it; a delta only records it.
    pub(crate) fn print_stderr_line(&self, line: Bound<'_, PyString>) -> PyResult<()> {
        if crate::bindings::_helpers::compile_context::delta::injection().is_none() {
            print_to(line.py(), "stderr", &line)?;
        }
        self.locked().0.push(line.unbind());
        Ok(())
    }

    /// Record the compile report; it is printed once the compile has been stored.
    pub(crate) fn record_stdout(&self, text: &Bound<'_, PyString>) -> PyResult<()> {
        let recorded: Bound<'_, PyString> = text.add("\n")?.downcast_into()?;
        self.locked().1 = Some(recorded.unbind());
        Ok(())
    }

    /// Print the recorded compile report, if any, exactly as it is stored.
    pub(crate) fn print_stdout(&self, py: Python<'_>) -> PyResult<()> {
        let Some(text) = self.locked().1.as_ref().map(|text| text.clone_ref(py)) else {
            return Ok(());
        };
        let stdout: Bound<'_, PyAny> = py.import("sys")?.getattr("stdout")?;
        let _ = stdout.call_method1("write", (text,))?;
        let _ = stdout.call_method0("flush")?;
        Ok(())
    }

    #[getter]
    pub(crate) fn stderr_lines(&self, py: Python<'_>) -> Vec<Py<PyString>> {
        self.locked()
            .0
            .iter()
            .map(|line| line.clone_ref(py))
            .collect()
    }

    #[getter]
    pub(crate) fn stdout(&self, py: Python<'_>) -> Option<Py<PyString>> {
        self.locked().1.as_ref().map(|text| text.clone_ref(py))
    }
}

/// The hit, miss and bypass counters plus the reuse check time, keyed as `compile_timings` is.
pub(crate) fn compile_reuse_timings<'py>(
    attempt: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = attempt.py();
    let outcome: String = native_field(attempt, "outcome")?.str()?.to_string();
    let timings: Bound<'py, PyDict> = PyDict::new(py);
    for (name, counted) in [
        (REUSE_HIT_TIMING, HIT_OUTCOME),
        (REUSE_MISS_TIMING, MISS_OUTCOME),
        (REUSE_BYPASS_TIMING, BYPASS_OUTCOME),
    ] {
        timings.set_item(name, i64::from(outcome == counted))?;
    }
    timings.set_item(REUSE_CHECK_TIMING, native_field(attempt, "check_ms")?)?;
    Ok(timings)
}

fn reuse_logger(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, "logging", "getLogger")?.call1((REUSE_LOGGER_NAME,))
}

/// Loaded module files, then the macro store's extra module paths, as Python lists them.
fn module_paths(py: Python<'_>) -> PyResult<Bound<'_, PyList>> {
    let paths: Bound<'_, PyList> = PyList::empty(py);
    let modules: Bound<'_, PyAny> = py.import("builtins")?.getattr("tuple")?.call1((py
        .import("sys")?
        .getattr("modules")?
        .call_method0("values")?,))?;
    let getattr: Bound<'_, PyAny> = py.import("builtins")?.getattr("getattr")?;
    for module in modules.try_iter()? {
        let path: Bound<'_, PyAny> = getattr.call1((module?, "__file__", py.None()))?;
        if path.is_instance_of::<pyo3::types::PyString>() {
            paths.append(path)?;
        }
    }
    let extra: Bound<'_, PyAny> = py
        .import("builtins")?
        .getattr("sorted")?
        .call1((host::entry(py, UserCodeHost::MacroStoreModulePaths)?.call0()?,))?;
    for path in extra.try_iter()? {
        let path: Bound<'_, PyAny> = path?;
        if path.is_instance_of::<pyo3::types::PyString>() {
            paths.append(path)?;
        }
    }
    Ok(paths)
}

/// The notice a long reuse record prints first, with when it started.
fn start_notice(py: Python<'_>, native: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    let pending: u64 = native.call_method0("pending_digest_bytes")?.extract()?;
    let paths: usize = native.call_method0("snapshot_paths")?.extract()?;
    if pending < REUSE_STORE_NOTICE_BYTES && paths < REUSE_STORE_NOTICE_PATHS {
        return Ok(None);
    }
    let mebibytes: f64 = pending as f64 / BYTES_PER_MEBIBYTE;
    let message: String =
        format!("Recording compile for reuse ({paths} paths, {mebibytes:.0} MiB to hash)...");
    print_to(py, "stderr", &message.into_bound_py_any(py)?)?;
    Ok(Some(imported(py, "time", "monotonic")?.call0()?.extract()?))
}

fn finish_notice(py: Python<'_>, started: Option<f64>, prefix: &str) -> PyResult<()> {
    let Some(started) = started else {
        return Ok(());
    };
    let now: f64 = imported(py, "time", "monotonic")?.call0()?.extract()?;
    let seconds: f64 = now - started;
    print_to(
        py,
        "stderr",
        &format!("{prefix}{seconds:.1} s)").into_bound_py_any(py)?,
    )
}

/// What `write_reusable_compile` records about one full compile.
struct CompileFacts<'py> {
    native: Bound<'py, PyAny>,
    output: Bound<'py, PyAny>,
    exit_code: Bound<'py, PyAny>,
    input_reads: Bound<'py, PyAny>,
    artifact_writes: Bound<'py, PyAny>,
    compile_cache_enabled: bool,
    artifacts_written: bool,
    dag_artifact_path: Bound<'py, PyAny>,
    json_output: Bound<'py, PyAny>,
}

/// Store a reusable compile natively; returns whether it was stored.
fn record<'py>(facts: &CompileFacts<'py>) -> PyResult<bool> {
    let py: Python<'py> = facts.native.py();
    let settings: Bound<'_, PyAny> = call_with(
        &host::entry(py, FacadeHost::ProviderSettingsInputs)?,
        &[(
            "settings_classes",
            native_field(&facts.input_reads, "settings_classes")?,
        )],
    )?;
    let unsupported: Bound<'_, PyAny> = native_field(&settings, "unsupported_reason")?;
    if !unsupported.is_none() {
        let _ = reuse_logger(py)?.call_method1(
            "debug",
            ("Compile reuse is off for this project: %s", &unsupported),
        )?;
    }
    let read_run_id: Bound<'_, PyAny> = native_field(&facts.input_reads, "read_run_id")?;
    let stdout: Bound<'_, PyAny> = native_field(&facts.output, "stdout")?;
    let eligible: bool = !stdout.is_none()
        && facts.compile_cache_enabled
        && !read_run_id.is_truthy()?
        && unsupported.is_none();
    let notice_started: Option<f64> = if eligible {
        start_notice(py, &facts.native)?
    } else {
        None
    };
    let strings = |value: Bound<'py, PyAny>| -> PyResult<Vec<String>> {
        value.try_iter()?.map(|item| item?.extract()).collect()
    };
    let settings_inputs: Option<Vec<SettingsRow>> = if unsupported.is_none() {
        let mut rows: Vec<SettingsRow> = Vec::new();
        for item in native_field(&settings, "inputs")?.try_iter()? {
            let item: Bound<'_, PyAny> = item?;
            rows.push(SettingsRow {
                case_sensitive: native_field(&item, "case_sensitive")?.extract()?,
                names: strings(native_field(&item, "names")?)?,
                prefixes: strings(native_field(&item, "prefixes")?)?,
                env_files: strings(native_field(&item, "env_files")?)?,
                secrets_dirs: strings(native_field(&item, "secrets_dirs")?)?,
            });
        }
        Some(rows)
    } else {
        None
    };
    let dag_path: Option<String> = if facts.dag_artifact_path.is_none() {
        None
    } else {
        Some(
            imported(py, "os.path", "abspath")?
                .call1((&facts.dag_artifact_path,))?
                .extract()?,
        )
    };
    let payload = CompileRecordPy {
        stdout: stdout.extract()?,
        stderr_lines: strings(native_field(&facts.output, "stderr_lines")?)?,
        exit_code: facts.exit_code.extract()?,
        compile_cache_enabled: facts.compile_cache_enabled,
        read_run_id: read_run_id.extract()?,
        settings_inputs,
        template_environment_names: strings(native_field(
            &facts.input_reads,
            "environment_names",
        )?)?,
        module_paths: module_paths(py)?.extract()?,
        artifacts: facts
            .artifact_writes
            .extract::<PyRef<'_, CompileArtifactWrites>>()?
            .artifact_rows(),
        artifacts_written: facts.artifacts_written,
        dag_path,
        json_output: facts.json_output.extract()?,
        rules: reuse_rules(py)?,
    };
    if let Some(mut capture) = crate::bindings::_helpers::compile_context::delta::take_capture() {
        capture.environment_names =
            native_field(&facts.input_reads, "environment_names")?.extract()?;
        let json_output: bool = facts.json_output.is_truthy()?;
        match crate::bindings::_helpers::compile_delta::image::ProjectImage::from_capture(
            capture,
            json_output,
        ) {
            Ok(image) => {
                let mut pending: Option<ImageSections> = None;
                if let Ok(sections) = image.encode() {
                    pending = Some(sections);
                }
                crate::bindings::_helpers::compile_context::delta::set_pending_image(pending);
            }
            Err(reason) => {
                crate::bindings::_helpers::compile_command::reuse_attempt::reuse_debug(
                    py,
                    &format!("Compile reuse kept no project image: {reason}"),
                )?;
            }
        }
    }
    let (outcome, digested): (&str, usize) = facts
        .native
        .extract::<PyRef<'_, NativeReuseAttempt>>()?
        .record(py, payload)?;
    let _ = reuse_logger(py)?.call_method1(
        "debug",
        ("Compile reuse record digested %d project files", digested),
    )?;
    finish_notice(
        py,
        notice_started,
        if outcome == SKIPPED {
            REUSE_STORE_SKIPPED_PREFIX
        } else {
            REUSE_STORE_DONE_PREFIX
        },
    )?;
    Ok(outcome == STORED)
}

/// Store a reusable full compile, or drop the stored one when it cannot be reused.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn write_reusable_compile(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<()> {
    let arguments: KeywordArguments<'_> = KeywordArguments::read(
        py,
        "write_reusable_compile",
        options,
        (
            &[
                "attempt",
                "output",
                "exit_code",
                "input_reads",
                "artifact_writes",
                "compile_cache_enabled",
                "artifacts_written",
                "dag_artifact_path",
                "json_output",
            ],
            &[],
        ),
    )?;
    let attempt: Bound<'_, PyAny> = arguments.value("attempt")?;
    let entry_path: Bound<'_, PyAny> = native_field(&attempt, "entry_path")?;
    let native: Bound<'_, PyAny> = native_field(&attempt, "native")?;
    if !native_field(&attempt, "outcome")?.eq(MISS_OUTCOME)?
        || entry_path.is_none()
        || native.is_none()
    {
        return Ok(());
    }
    let facts: CompileFacts<'_> = CompileFacts {
        native,
        output: arguments.value("output")?,
        exit_code: arguments.value("exit_code")?,
        input_reads: arguments.value("input_reads")?,
        artifact_writes: arguments.value("artifact_writes")?,
        compile_cache_enabled: arguments.flag("compile_cache_enabled")?,
        artifacts_written: arguments.flag("artifacts_written")?,
        dag_artifact_path: arguments.value("dag_artifact_path")?,
        json_output: arguments.value("json_output")?,
    };
    let stored: bool = match record(&facts) {
        Ok(stored) => stored,
        Err(error) if error.is_instance_of::<PyException>(py) => {
            let _ = reuse_logger(py)?.call_method(
                "debug",
                ("Compile reuse did not store this compile",),
                Some(&keywords(
                    py,
                    &[("exc_info", error.value(py).clone().into_any())],
                )?),
            )?;
            let unlinked: PyResult<Bound<'_, PyAny>> = entry_path.call_method(
                "unlink",
                (),
                Some(&keywords(
                    py,
                    &[("missing_ok", true.into_bound_py_any(py)?)],
                )?),
            );
            if let Err(error) = unlinked
                && !error.is_instance_of::<PyOSError>(py)
            {
                return Err(error);
            }
            false
        }
        Err(error) => return Err(error),
    };
    if stored {
        write_launch_record(&attempt)?;
    }
    Ok(())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CompileArtifactWrites>()?;
    module.add_class::<RecordedCompileOutput>()?;
    module.add_function(wrap_pyfunction!(write_reusable_compile, module)?)
}
