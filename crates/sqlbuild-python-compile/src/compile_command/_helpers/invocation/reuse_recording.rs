//! What one full compile printed and left in each artifact, and storing it for compile reuse.

use crate::compile_command::main::invocation::print_to::print_to;
use crate::compile_command::main::invocation::reuse_debug::reuse_debug;
use crate::compile_command::models::CompileArtifactWrites;
use crate::compile_command::models::CompileFacts;
use crate::compile_command::types::ArtifactRecorder;
use sqlbuild_cache::project_reuse::types::ImageSections;
use std::path::{Path, PathBuf};

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyRef, PyResult, Python};
use pyo3::types::{PyList, PyListMethods, PyString};

use sqlbuild_cache::project_reuse::models::RecordedArtifact;
use sqlbuild_core::posix_path::main::posix_abspath::posix_abspath;
use sqlbuild_host::entries::models::{FacadeHost, UserCodeHost};

use crate::compile_command::_helpers::invocation::reuse_attempt::reuse_rules;
use crate::compile_command::constants::BYTES_PER_MEBIBYTE;
use crate::compile_command::constants::REUSE_LOGGER_NAME;
use crate::compile_command::constants::REUSE_STORE_DONE_PREFIX;
use crate::compile_command::constants::REUSE_STORE_NOTICE_BYTES;
use crate::compile_command::constants::REUSE_STORE_NOTICE_PATHS;
use crate::compile_command::constants::REUSE_STORE_SKIPPED_PREFIX;
use crate::compile_outputs::main::artifact_digest::artifact_digest;
use crate::compile_outputs::models::CompileRecordPy;
use crate::compile_outputs::models::NativeReuseAttempt;
use crate::compile_outputs::models::SettingsRow;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) const MISS_OUTCOME: &str = "miss";
pub(crate) const HIT_OUTCOME: &str = "hit";
pub(crate) const BYPASS_OUTCOME: &str = "bypass";
const STORED: &str = "stored";
const SKIPPED: &str = "skipped";

/// `os.path.abspath(path)`, computed natively on POSIX for UTF-8 paths and working directories.
pub(crate) fn absolute(path: &Bound<'_, PyAny>) -> PyResult<String> {
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
pub(crate) type PrintedOutput = (Vec<Py<PyString>>, Option<Py<PyString>>);

pub(crate) fn reuse_logger(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
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
        .call1((host_entry(py, UserCodeHost::MacroStoreModulePaths)?.call0()?,))?;
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

/// Store a reusable compile natively; returns whether it was stored.
pub(crate) fn record<'py>(facts: &CompileFacts<'py>) -> PyResult<bool> {
    let py: Python<'py> = facts.native.py();
    let settings: Bound<'_, PyAny> = call_with(
        &host_entry(py, FacadeHost::ProviderSettingsInputs)?,
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
    if let Some(mut capture) =
        sqlbuild_python_base::compile_context::main::delta::take_capture::take_capture()
    {
        capture.environment_names =
            native_field(&facts.input_reads, "environment_names")?.extract()?;
        let json_output: bool = facts.json_output.is_truthy()?;
        match crate::compile_delta::models::ProjectImage::from_capture(capture, json_output) {
            Ok(image) => {
                let mut pending: Option<ImageSections> = None;
                if let Ok(sections) = image.encode() {
                    pending = Some(sections);
                }
                sqlbuild_python_base::compile_context::main::delta::set_pending_image::set_pending_image(pending);
            }
            Err(reason) => {
                reuse_debug(
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
