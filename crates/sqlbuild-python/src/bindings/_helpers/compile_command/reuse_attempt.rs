//! Check whether a compile can replay the stored result, replay it on a hit and leave a launch record.

use std::path::PathBuf;

use pyo3::exceptions::PyOSError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyList, PyString};
use pyo3::{IntoPyObject, IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_cache::project_reuse::constants::FAILED_COMPILED_DIRECTORY_NAME;
use sqlbuild_cli::replay::main::output_file_identities::output_file_identities;
use sqlbuild_cli::replay::main::write_launch_record::write_launch_record as write_record_file;
use sqlbuild_core::command_output::constants::INVOCATION_CONTEXT_ENV;

use sqlbuild_cli::replay::constants::{LAUNCH_RECORD_DIRECTORY, LAUNCH_RECORD_FORMAT};
use sqlbuild_host::entries::models::{ExecutorHost, FacadeHost};

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported, keywords};
use crate::bindings::_helpers::build_progress::progress::{
    NativeProgressProjector, current_projector,
};
use crate::bindings::_helpers::cli::launch_state::launch_record_path;
use crate::bindings::_helpers::compile_command::constants::{
    COMPILE_CACHE_DISABLE_ENV_VAR, COMPILE_CACHE_DISABLE_VALUE, COMPILE_COMMAND_NAME,
    COMPILED_DIRECTORY_NAME, COMPUTE_LOG_DIRECTORY_NAME, EXCLUDED_DIRECTORIES,
    EXCLUDED_ROOT_ATTRIBUTE, EXCLUDED_ROOT_DIRECTORIES, MILLISECONDS_PER_SECOND,
    MISSING_ENVIRONMENT_VALUE, MISSING_FILE_DIGEST, MISSING_PATH_MTIME_NS,
    NATIVE_REUSE_DIRECTORY_NAME, PRESENCE_ONLY_FILE_SUFFIXES, PROJECT_CONFIG_FILENAMES,
    PROJECT_ROOT_PATH_MTIME_NS, RACY_WINDOW_ATTRIBUTE, RACY_WINDOW_NS,
    RETIRED_REUSE_DIRECTORY_NAMES, REUSE_BYPASS_TIMING, REUSE_CHECK_TIMING, REUSE_DISABLE_ENV_VAR,
    REUSE_DISABLE_VALUE, REUSE_HIT_MESSAGE, REUSE_HIT_TIMING, REUSE_LOGGER_NAME,
    REUSE_MAX_ENTRY_BYTES, REUSE_MAX_STORED_ENTRIES, REUSE_MISS_TIMING, TARGET_DIRECTORY_NAME,
    TOTAL_TIMING, TRACKED_ENVIRONMENT_PREFIXES, UNTRACKED_ENVIRONMENT_NAMES,
};
use crate::bindings::_helpers::compile_command::models::CompileReuseAttempt;
use crate::bindings::_helpers::compile_command::runtime_identity::{
    invocation_identity, runtime_identity,
};
use crate::bindings::_helpers::compile_context::delta;
use crate::bindings::_helpers::compile_delta::driver::{
    self, DELTA_VERIFY_ENV_VAR, DeltaOutcome, delta_enabled, env_enabled,
};
use crate::bindings::_helpers::compile_delta::verify;
use crate::bindings::_helpers::compile_driver::command::run_compile_command;
use crate::bindings::_helpers::compile_driver::phases::{elapsed_ms, monotonic};
use crate::bindings::_helpers::compile_outputs::json_report::plain_json;
use crate::bindings::_helpers::compile_outputs::reuse::{
    ReuseCheckPy, ReuseRulesPy, check_compile_reuse,
};
use crate::bindings::_helpers::frontier::cache_directory::cache_directory;
use crate::bindings::_helpers::macro_bridge::first_compile::keep_delta_claim;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::presentation_text::terminal::stdout_supports_color;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::NativeAttr;
use crate::bindings::_helpers::records::registry;

const REUSE_REQUEST: FacadeHost = FacadeHost::CompileReuseRequest;
const HIT: &str = "hit";
const CHECKED_HIT: &str = "hit";
const CHECKED_BYPASS: &str = "bypass";
const MISS: &str = "miss";
/// The logging format that prints its one argument as given.
const LOG_TEXT: &str = "%s";
const BYPASS: &str = "bypass";

fn reuse_logger(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, "logging", "getLogger")?.call1((REUSE_LOGGER_NAME,))
}

/// Log one message at debug level on the compile reuse logger.
pub(crate) fn reuse_debug(py: Python<'_>, message: &str) -> PyResult<()> {
    reuse_debug_with(py, LOG_TEXT, message)
}

/// Log `argument` through the logging `format` at debug level on the compile reuse logger.
fn reuse_debug_with<'py>(
    py: Python<'py>,
    format: &str,
    argument: impl pyo3::IntoPyObject<'py>,
) -> PyResult<()> {
    let _ = reuse_logger(py)?.call_method1("debug", (format, argument))?;
    Ok(())
}

/// The reuse constants the native check, the stored compile and the launch record share.
pub(crate) fn reuse_rules(py: Python<'_>) -> PyResult<ReuseRulesPy> {
    let mut excluded_root: Vec<String> =
        registry::native(py, NativeAttr::CompileReuseExcludedRootDirectories)?
            .try_iter()?
            .map(|value| value?.extract())
            .collect::<PyResult<_>>()?;
    excluded_root.sort();
    Ok(ReuseRulesPy {
        excluded: EXCLUDED_DIRECTORIES.map(str::to_owned).into(),
        excluded_root,
        presence_suffixes: PRESENCE_ONLY_FILE_SUFFIXES.map(str::to_owned).into(),
        output_files: output_file_identities(),
        tracked_prefixes: TRACKED_ENVIRONMENT_PREFIXES.map(str::to_owned).into(),
        untracked_names: UNTRACKED_ENVIRONMENT_NAMES.map(str::to_owned).into(),
        disabling_variables: vec![
            (
                COMPILE_CACHE_DISABLE_ENV_VAR.to_owned(),
                COMPILE_CACHE_DISABLE_VALUE.to_owned(),
            ),
            (
                REUSE_DISABLE_ENV_VAR.to_owned(),
                REUSE_DISABLE_VALUE.to_owned(),
            ),
        ],
        config_filenames: PROJECT_CONFIG_FILENAMES.map(str::to_owned).into(),
        racy_window_ns: registry::native(py, NativeAttr::CompileReuseRacyWindowNs)?.extract()?,
        max_stored_entries: REUSE_MAX_STORED_ENTRIES as usize,
        max_entry_bytes: REUSE_MAX_ENTRY_BYTES as usize,
        missing_environment_value: MISSING_ENVIRONMENT_VALUE.to_owned(),
        missing_file_digest: MISSING_FILE_DIGEST.to_owned(),
        missing_path_mtime_ns: MISSING_PATH_MTIME_NS,
        project_root_path_mtime_ns: PROJECT_ROOT_PATH_MTIME_NS,
        compiled_root: PathBuf::from(TARGET_DIRECTORY_NAME).join(COMPILED_DIRECTORY_NAME),
        retired_directories: RETIRED_REUSE_DIRECTORY_NAMES.map(str::to_owned).into(),
    })
}

/// The launch record `sqb` replays this reusable compile from, as a JSON object.
fn launch_payload<'py>(
    attempt: &Bound<'py, PyAny>,
    native: &Bound<'py, PyAny>,
    key: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = attempt.py();
    let (invocation_digest, runtime, search_path): (
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
    ) = native.call_method0("launch_identity")?.extract()?;
    let project_dir: Bound<'py, PyAny> = native_field(attempt, "project_dir")?;
    let resolved: Bound<'py, PyAny> = project_dir.call_method0("resolve")?;
    let capture: Bound<'py, PyAny> = if py
        .import("sys")?
        .getattr("stdout")?
        .is_instance(&host::entry(py, ExecutorHost::TextComputeLogTee)?)?
    {
        let logs: Bound<'py, PyAny> = resolved
            .call_method1("__truediv__", (COMPUTE_LOG_DIRECTORY_NAME,))?
            .call_method0("resolve")?;
        let capture: Bound<'py, PyDict> = PyDict::new(py);
        capture.set_item("root", logs.str()?)?;
        capture.set_item("command", COMPILE_COMMAND_NAME)?;
        capture.set_item("project_dir", resolved.str()?)?;
        capture.set_item("target", native_field(attempt, "selected_target")?)?;
        capture.into_any()
    } else {
        py.None().into_bound(py)
    };
    let timings: Bound<'py, PyDict> = PyDict::new(py);
    for (key, name) in [
        ("hit", REUSE_HIT_TIMING),
        ("miss", REUSE_MISS_TIMING),
        ("bypass", REUSE_BYPASS_TIMING),
        ("check", REUSE_CHECK_TIMING),
        ("total", TOTAL_TIMING),
    ] {
        timings.set_item(key, name)?;
    }
    let payload: Bound<'py, PyDict> = PyDict::new(py);
    payload.set_item("format", LAUNCH_RECORD_FORMAT)?;
    payload.set_item("key", key)?;
    payload.set_item("project_dir", project_dir.str()?)?;
    payload.set_item("store_path", native.call_method0("store_path")?)?;
    payload.set_item("invocation_digest", invocation_digest)?;
    payload.set_item("runtime", runtime)?;
    payload.set_item("search_path", search_path)?;
    payload.set_item(
        "startup_inputs",
        host::entry(py, FacadeHost::StartupInputs)?.call0()?,
    )?;
    payload.set_item("json_output", native_field(attempt, "json_output")?)?;
    let native_rules: ReuseRulesPy = reuse_rules(py)?;
    let compiled_root = native_rules.compiled_root.clone().into_os_string();
    let rules: Bound<'py, PyDict> = native_rules.into_pyobject(py)?;
    rules.set_item("compiled_root", compiled_root)?;
    rules.del_item("output_files")?;
    payload.set_item("rules", rules)?;
    payload.set_item("capture", capture)?;
    payload.set_item("hit_message", PyList::new(py, REUSE_HIT_MESSAGE)?)?;
    payload.set_item("timings", timings)?;
    Ok(payload)
}

fn write_record(
    attempt: &Bound<'_, PyAny>,
    native: &Bound<'_, PyAny>,
    path: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let payload: Bound<'_, PyDict> = launch_payload(attempt, native, native_field(path, "stem")?)?;
    let text: String = plain_json(payload.as_any())?;
    let path: PathBuf = path.extract()?;
    write_record_file(&path, &text).map_err(PyErr::from)
}

/// Record this reusable compile for `sqb` when nothing it cannot replay took part.
pub(crate) fn write_launch_record(attempt: &Bound<'_, PyAny>) -> PyResult<()> {
    let py: Python<'_> = attempt.py();
    let path: Bound<'_, PyAny> = launch_record_path(py)?;
    let native: Bound<'_, PyAny> = native_field(attempt, "native")?;
    if path.is_none() || native.is_none() {
        return Ok(());
    }
    let environment: Bound<'_, PyAny> = imported(py, "os", "environ")?;
    if environment.contains(INVOCATION_CONTEXT_ENV)? {
        return Ok(());
    }
    let mut directory: Bound<'_, PyAny> = native_field(attempt, "project_dir")?;
    for part in LAUNCH_RECORD_DIRECTORY {
        directory = directory.call_method1("__truediv__", (part,))?;
    }
    if native_field(&path, "parent")?
        .call_method0("resolve")?
        .ne(directory.call_method0("resolve")?)?
    {
        return Ok(());
    }
    match write_record(attempt, &native, &path) {
        Err(error) if error.is_instance_of::<PyOSError>(py) => {
            reuse_logger(py)?.call_method(
                "debug",
                ("Compile reuse did not leave a launch record",),
                Some(&keywords(
                    py,
                    &[("exc_info", error.value(py).clone().into_any())],
                )?),
            )?;
            Ok(())
        }
        written => written,
    }
}

fn outcome<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    name.into_bound_py_any(py)
}

/// A `CompileReuseAttempt` with `fields`.
fn reuse_attempt<'py>(
    py: Python<'py>,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    call_with(py.get_type::<CompileReuseAttempt>().as_any(), fields)
}

/// The native reuse check request for this invocation.
fn reuse_check<'py>(
    request: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<ReuseCheckPy> {
    let py: Python<'py> = request.py();
    let flag = |name: &str| -> PyResult<bool> { native_field(request, name)?.is_truthy() };
    let use_color: bool = !flag("no_color")? && stdout_supports_color(py)?;
    let directory: PathBuf =
        cache_directory(&imported(py, "pathlib", "Path")?.call1((project_dir,))?)?.extract()?;
    Ok(ReuseCheckPy {
        project_dir: project_dir.extract()?,
        store_directory: directory.join(NATIVE_REUSE_DIRECTORY_NAME),
        selected_target: native_field(request, "selected_target")?.extract()?,
        invocation: invocation_identity(request, project_dir, use_color)?.extract()?,
        runtime: runtime_identity(py)?,
        search_path: py.import("sys")?.getattr("path")?.extract()?,
        bypass_requested: flag("no_cache")?
            || flag("manifest")?
            || flag("profiling")?
            || flag("debug")?,
        json_output: native_field(request, "json_output")?.extract()?,
        rules: reuse_rules(py)?,
    })
}

/// `print(text, file=sys.stderr)`.
fn print_stderr(py: Python<'_>, text: &Bound<'_, PyAny>) -> PyResult<()> {
    py.import("builtins")?.getattr("print")?.call(
        (text,),
        Some(&keywords(
            py,
            &[("file", py.import("sys")?.getattr("stderr")?)],
        )?),
    )?;
    Ok(())
}

/// `fields` with each override replacing the field of the same name, as `dataclasses.replace`.
fn overridden<'py>(
    mut fields: Vec<(&'static str, Bound<'py, PyAny>)>,
    overrides: Vec<(&'static str, Bound<'py, PyAny>)>,
) -> Vec<(&'static str, Bound<'py, PyAny>)> {
    for (name, value) in overrides {
        match fields.iter_mut().find(|field| field.0 == name) {
            Some(field) => field.1 = value,
            None => fields.push((name, value)),
        }
    }
    fields
}

/// Replay the stored compile on a hit; the missed attempt when it cannot be replayed.
fn replayed<'py>(
    native: &Bound<'py, PyAny>,
    missed: Vec<(&'static str, Bound<'py, PyAny>)>,
    (started, json_output): (f64, bool),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = native.py();
    let check_ms: i64 = elapsed_ms(py, started)?;
    let timings: Vec<(&str, i64)> = vec![
        (REUSE_HIT_TIMING, 1),
        (REUSE_MISS_TIMING, 0),
        (REUSE_BYPASS_TIMING, 0),
        (REUSE_CHECK_TIMING, check_ms),
        (TOTAL_TIMING, elapsed_ms(py, started)?),
    ];
    let rows: Bound<'py, PyList> = if json_output {
        PyList::new(py, timings)?
    } else {
        PyList::empty(py)
    };
    let replay: Bound<'py, PyAny> = native.call_method1("replay", (rows,))?;
    if replay.is_none() {
        return reuse_attempt(
            py,
            &overridden(missed, vec![("check_ms", 0_i64.into_bound_py_any(py)?)]),
        );
    }
    let (stdout, stderr_lines, exit_code): (
        Bound<'py, PyString>,
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
    ) = replay.extract()?;
    let [prefix, suffix] = REUSE_HIT_MESSAGE;
    let seconds: f64 = check_ms as f64 / MILLISECONDS_PER_SECOND;
    print_stderr(
        py,
        &format!("{prefix}{seconds:.1}{suffix}").into_bound_py_any(py)?,
    )?;
    for line in stderr_lines.try_iter()? {
        print_stderr(py, &line?)?;
    }
    let output: Bound<'py, PyAny> = py.import("sys")?.getattr("stdout")?;
    output.call_method1("write", (stdout,))?;
    output.call_method0("flush")?;
    reuse_attempt(
        py,
        &overridden(
            missed,
            vec![
                ("outcome", outcome(py, HIT)?),
                ("check_ms", check_ms.into_bound_py_any(py)?),
                ("exit_code", exit_code),
            ],
        ),
    )
}

/// Replay the stored compile when every input matches, otherwise prepare to store one.
#[pyfunction]
#[pyo3(signature = (*, request))]
fn attempt_compile_reuse<'py>(request: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let started: f64 = monotonic(py)?;
    let requested: Bound<'py, PyAny> = native_field(request, "project_dir")?;
    let project_dir: Bound<'py, PyAny> =
        imported(py, "os.path", "abspath")?.call1((if requested.is_none() {
            imported(py, "os", "getcwd")?.call0()?
        } else {
            requested
        },))?;
    let native: Bound<'py, PyAny> = check_compile_reuse(py, reuse_check(request, &project_dir)?)
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let checked: String = native.call_method0("outcome")?.extract()?;
    let json_output: Bound<'py, PyAny> = native_field(request, "json_output")?;
    let mut base: Vec<(&'static str, Bound<'py, PyAny>)> = vec![
        (
            "project_dir",
            imported(py, "pathlib", "Path")?.call1((&project_dir,))?,
        ),
        ("started", started.into_bound_py_any(py)?),
        ("selected_target", native_field(request, "selected_target")?),
        ("json_output", json_output.clone()),
    ];
    if checked == CHECKED_BYPASS {
        base.push(("outcome", outcome(py, BYPASS)?));
        return reuse_attempt(py, &base);
    }
    reuse_debug_with(
        py,
        "Compile reuse check digested %d project files",
        native.call_method0("digested_files")?,
    )?;
    let check_ms: i64 = if native.call_method0("replay_failed")?.is_truthy()? {
        0
    } else {
        elapsed_ms(py, started)?
    };
    base.extend([
        ("outcome", outcome(py, MISS)?),
        (
            "entry_path",
            imported(py, "pathlib", "Path")?.call1((native.call_method0("store_path")?,))?,
        ),
        ("native", native.clone()),
        ("check_ms", check_ms.into_bound_py_any(py)?),
    ]);
    if checked != CHECKED_HIT {
        return reuse_attempt(py, &base);
    }
    let attempt: Bound<'py, PyAny> = replayed(&native, base, (started, json_output.is_truthy()?))?;
    if native_field(&attempt, "outcome")?.eq(outcome(py, HIT)?)? {
        write_launch_record(&attempt)?;
    }
    Ok(attempt)
}

/// The compile command choices that decide whether and how a compile is reused.
fn reuse_request<'py>(request: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let flags: Bound<'py, PyAny> = native_field(request, "profile_flags")?;
    let mut profiling: bool = false;
    for name in [
        "skip_discovery_sql_analysis",
        "skip_column_inference",
        "skip_contracts",
        "skip_write",
    ] {
        profiling = profiling || native_field(&flags, name)?.is_truthy()?;
    }
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = Vec::new();
    for name in [
        "project_dir",
        "selected_target",
        "json_output",
        "no_color",
        "manifest",
        "dag_path",
        "defer_to",
        "no_cache",
        "no_sql_validation",
    ] {
        fields.push((name, native_field(request, name)?));
    }
    fields.push((
        "lineage_mode",
        native_field(request, "lineage_mode")?
            .getattr("value")?
            .str()?
            .into_any(),
    ));
    for name in ["select", "exclude", "cli_vars", "debug"] {
        fields.push((name, native_field(request, name)?));
    }
    fields.push(("profiling", profiling.into_bound_py_any(py)?));
    call_with(&host::entry(py, REUSE_REQUEST)?, &fields)
}

/// Run one compile command request, replaying the previous compile when nothing changed.
#[pyfunction]
pub(crate) fn run_compile<'py>(request: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let attempt: Bound<'py, PyAny> = attempt_compile_reuse(&reuse_request(request)?)?;
    let exit_code: Bound<'py, PyAny> = native_field(&attempt, "exit_code")?;
    if native_field(&attempt, "outcome")?.eq(outcome(py, HIT)?)? && !exit_code.is_none() {
        return Ok(exit_code);
    }
    let missed: bool = native_field(&attempt, "outcome")?.eq(outcome(py, MISS)?)?;
    let code: PyResult<i64> = if missed && delta_enabled() {
        compiled_with_delta(request, &attempt)
    } else {
        run_compile_command(py, request.clone(), Some(attempt))
    };
    delta::reset();
    verify::disarm();
    code.and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))
}

/// Compile the changed files as a delta when it can, otherwise (or to verify it) in full.
fn compiled_with_delta<'py>(
    request: &Bound<'py, PyAny>,
    attempt: &Bound<'py, PyAny>,
) -> PyResult<i64> {
    let py: Python<'py> = request.py();
    let verifying: bool = env_enabled(DELTA_VERIFY_ENV_VAR);
    let started: f64 = monotonic(py)?;
    let projector: Option<pyo3::Py<NativeProgressProjector>> = current_projector(py)?;
    if let Some(projector) = &projector {
        projector.get().hold_lines();
    }
    let ran: PyResult<DeltaOutcome<'py>> = driver::run(request, attempt);
    let held: Vec<String> = projector
        .as_ref()
        .map(|projector| projector.get().release_lines())
        .unwrap_or_default();
    let delta: DeltaOutcome<'py> = match ran {
        Ok(delta) => delta,
        Err(error) => DeltaOutcome::Fallback(format!("the delta compile failed: {error}")),
    };
    let write_held = || -> PyResult<()> {
        match &projector {
            Some(projector) => projector.get().write_held(py, &held),
            None => Ok(()),
        }
    };
    delta::reset();
    let project_dir: Bound<'py, PyAny> = native_field(attempt, "project_dir")?;
    let cache: Bound<'py, PyAny> = cache_directory(&project_dir)?;
    let compiled_root: PathBuf = std::path::absolute(
        project_dir
            .extract::<PathBuf>()?
            .join(TARGET_DIRECTORY_NAME)
            .join(COMPILED_DIRECTORY_NAME),
    )?;
    let failed_root: PathBuf = compiled_root.with_file_name(FAILED_COMPILED_DIRECTORY_NAME);
    match delta {
        DeltaOutcome::Finished(finished) if !verifying => {
            keep_delta_claim();
            write_held()?;
            let (files, models) = (finished.changed_files, finished.rerun_models);
            let committing: std::time::Instant = std::time::Instant::now();
            let code: i64 = driver::commit(*finished, (request, attempt))?;
            driver::trace(py, "delta record and print", committing)?;
            let seconds: f64 = elapsed_ms(py, started)? as f64 / MILLISECONDS_PER_SECOND;
            print_stderr(
                py,
                &format!(
                    "Compiled {files} changed file(s) against the previous compile ({models} model(s), {seconds:.1}s)"
                )
                .into_bound_py_any(py)?,
            )?;
            return Ok(code);
        }
        DeltaOutcome::Raised(error) if !verifying => {
            keep_delta_claim();
            write_held()?;
            return Err(error);
        }
        DeltaOutcome::Fallback(reason) => {
            reuse_debug(py, &format!("Delta compile fell back: {reason}"))?;
            if verifying {
                print_stderr(
                    py,
                    &format!("delta compile fell back: {reason}").into_bound_py_any(py)?,
                )?;
            }
            delta::start_capture();
            return run_compile_command(py, request.clone(), Some(attempt.clone()));
        }
        DeltaOutcome::Finished(finished) => verify::hold_expected(verify::Expected {
            stdout: Some(finished.stdout.clone()),
            exit_code: Some(finished.phases.0.exit_code),
            artifacts_written: finished.phases.0.artifacts_written,
            raised: None,
            compiled: verify::compiled_digests(&compiled_root),
            compiled_root: compiled_root.to_string_lossy().into_owned(),
            failed: verify::compiled_digests(&failed_root),
            failed_root: failed_root.to_string_lossy().into_owned(),
        }),
        DeltaOutcome::Raised(error) => verify::hold_expected(verify::Expected {
            raised: Some(error.to_string()),
            compiled_root: compiled_root.to_string_lossy().into_owned(),
            failed_root: failed_root.to_string_lossy().into_owned(),
            ..verify::Expected::default()
        }),
    }
    delta::start_capture();
    let full: PyResult<i64> = run_compile_command(py, request.clone(), Some(attempt.clone()));
    let raised: Option<String> = full.as_ref().err().map(ToString::to_string);
    let differences: Vec<String> = verify::differences(raised);
    if differences.is_empty() {
        print_stderr(py, &"delta compile verified".into_bound_py_any(py)?)?;
        return full;
    }
    let cache_dir: PathBuf = cache.extract()?;
    verify::write_mismatch(&cache_dir, &differences);
    for difference in &differences {
        print_stderr(
            py,
            &format!("delta compile mismatch: {difference}").into_bound_py_any(py)?,
        )?;
    }
    full?;
    Ok(verify::MISMATCH_EXIT_CODE)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py: Python<'_> = module.py();
    module.add(RACY_WINDOW_ATTRIBUTE, RACY_WINDOW_NS)?;
    module.add(
        EXCLUDED_ROOT_ATTRIBUTE,
        PyFrozenSet::new(py, EXCLUDED_ROOT_DIRECTORIES)?,
    )?;
    module.add_function(wrap_pyfunction!(attempt_compile_reuse, module)?)?;
    module.add_function(wrap_pyfunction!(run_compile, module)?)
}
