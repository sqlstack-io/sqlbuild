//! The project's Python tests, which a local `sqb test` runs with pytest after its SQL tests.

use std::collections::BTreeSet;
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::Instant;

use pyo3::exceptions::PyOSError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_core::cli_text::main::format_phase_line::format_phase_line;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_discovery::python_paths::main::python_test_paths::python_test_paths;

use crate::bindings::_helpers::compile_driver::pipeline_entries::project_graph;
use crate::bindings::_helpers::graph::python_node_graph::run_selection_from_inputs;
use crate::bindings::_helpers::planner_selection::unit_tests::split_project_unit_test_selectors;
use crate::bindings::_helpers::records::construct::new_helped_error;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::ErrorId;

/// The prefix of a Python test module's file name.
const TEST_MODULE_PREFIX: &str = "test_";
const PYTEST_MODULE: &str = "pytest";
/// The diagnostic code of a Python test run that cannot start.
pub(crate) const PYTHON_TESTS_INPUT_CODE: &str = "S000";
/// The project folder whose `python/` and `providers/` folders mirror the project's.
const TESTS_FOLDER: &str = "tests";
/// The discovered Python node collections whose folders a selection's tests mirror.
const NODE_COLLECTIONS: [&str; 4] = [
    "loader_functions",
    "task_functions",
    "asset_functions",
    "check_functions",
];
/// Lets tests import project and `tests.*` modules, and mirrored folders reuse module names.
const IMPORT_MODE: &str = "--import-mode=importlib";

/// Whether the request selects or excludes anything.
pub(crate) fn selective(request: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(request.getattr("select")?.is_truthy()? || request.getattr("exclude")?.is_truthy()?)
}

/// The Python test modules a local `sqb test` runs: all, or those mirroring the selected Python nodes.
pub(crate) fn selected_python_tests(
    request: &Bound<'_, PyAny>,
    invocation: &Bound<'_, PyAny>,
    pipeline: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<String>> {
    let py: Python<'_> = request.py();
    let project_dir: PathBuf = native_field(invocation, "effective_project_dir")?.extract()?;
    let modules: Vec<String> = python_test_paths(&project_dir)
        .into_iter()
        .filter(|parts| {
            parts
                .last()
                .is_some_and(|name| name.starts_with(TEST_MODULE_PREFIX))
        })
        .map(|parts| parts.join("/"))
        .collect();
    let selected: Vec<String> = match pipeline {
        Some(pipeline) if !modules.is_empty() && selective(request)? => {
            let folders: BTreeSet<String> = mirrored_folders(
                request,
                pipeline,
                &native_field(invocation, "discovered_inputs")?,
            )?;
            let mut mirrored: Vec<String> = Vec::new();
            for module in modules {
                if folders.iter().any(|folder| module.starts_with(folder)) {
                    mirrored.push(module);
                }
            }
            mirrored
        }
        _ => modules,
    };
    if native_field(request, "python_only")?.is_truthy()? && selected.is_empty() {
        return Err(new_helped_error(
            py,
            ErrorId::PlannerInput,
            ("no Python tests found".to_owned(), PYTHON_TESTS_INPUT_CODE),
            Some(
                "add test_*.py modules under tests/python/ or tests/providers/, mirroring the code they test",
            ),
        ));
    }
    if !selected.is_empty()
        && py
            .import("importlib.util")?
            .call_method1("find_spec", (PYTEST_MODULE,))?
            .is_none()
    {
        return Err(new_helped_error(
            py,
            ErrorId::PlannerInput,
            (
                "Python tests found but pytest isn't installed".to_owned(),
                PYTHON_TESTS_INPUT_CODE,
            ),
            Some("install the dev dependencies, or run `sqb test --sql`"),
        ));
    }
    Ok(selected)
}

/// Run `modules` with this interpreter's pytest from the project folder; pytest's exit code.
pub(crate) fn run_python_tests(
    request: &Bound<'_, PyAny>,
    invocation: &Bound<'_, PyAny>,
    modules: &[String],
) -> PyResult<i32> {
    let py: Python<'_> = request.py();
    let stream: Bound<'_, PyAny> = invocation.getattr("progress_stream")?;
    let style: CliStyle = CliStyle {
        use_color: invocation.getattr("use_color")?.is_truthy()?,
    };
    let project_dir: PathBuf = native_field(invocation, "effective_project_dir")?.extract()?;
    let interpreter: PathBuf = py.import("sys")?.getattr("executable")?.extract()?;
    let files: String = match modules.len() {
        1 => "1 file".to_owned(),
        count => format!("{count} files"),
    };
    stream.call_method1(
        "write",
        (format!("\nRunning Python tests with pytest  ({files})\n"),),
    )?;
    stream.call_method0("flush")?;
    py.import("sys")?.getattr("stdout")?.call_method0("flush")?;
    let mut command: Command = Command::new(&interpreter);
    command
        .args(["-m", PYTEST_MODULE, IMPORT_MODE])
        .args(modules)
        .current_dir(&project_dir)
        .env("PYTHONPATH", python_path(&project_dir)?)
        .stdin(Stdio::null());
    if native_field(request, "json_output")?.is_truthy()? {
        command.stdout(Stdio::from(std::io::stderr().as_fd().try_clone_to_owned()?));
    }
    let started: Instant = Instant::now();
    let status: ExitStatus = py
        .detach(|| command.status())
        .map_err(|error| PyOSError::new_err(format!("cannot run pytest: {error}")))?;
    let elapsed: f64 = started.elapsed().as_secs_f64();
    let code: i32 = status.code().unwrap_or(1);
    let line: String = if code == 0 {
        format_phase_line(
            style,
            true,
            ("Python tests passed", &format!("{files}  ({elapsed:.2}s)")),
        )
    } else {
        format_phase_line(
            style,
            false,
            (
                "Python tests failed",
                &format!("pytest exit code {code}  ({elapsed:.2}s)"),
            ),
        )
    };
    stream.call_method1("write", (format!("{line}\n"),))?;
    stream.call_method0("flush")?;
    Ok(code)
}

/// The `tests/` folders mirroring the Python nodes the request's resource selectors resolve to.
fn mirrored_folders(
    request: &Bound<'_, PyAny>,
    pipeline: &Bound<'_, PyAny>,
    discovered: &Bound<'_, PyAny>,
) -> PyResult<BTreeSet<String>> {
    let split: Bound<'_, PyAny> = split_project_unit_test_selectors(
        &request.getattr("select")?,
        &request.getattr("exclude")?,
        discovered,
        true,
    )?;
    let select: Bound<'_, PyAny> = native_field(&split, "select")?;
    let exclude: Bound<'_, PyAny> = native_field(&split, "exclude")?;
    if !select.is_truthy()? && request.getattr("select")?.is_truthy()? {
        return Ok(BTreeSet::new());
    }
    let selection: Bound<'_, PyAny> = run_selection_from_inputs(
        (select.extract()?, exclude.extract()?),
        &project_graph(&native_field(pipeline, "project")?)?,
        discovered,
    )?;
    let names: Bound<'_, PyAny> = native_field(&selection, "python_node_names")?;
    let mut folders: BTreeSet<String> = BTreeSet::new();
    for collection in NODE_COLLECTIONS {
        for node in native_field(discovered, collection)?.try_iter()? {
            let node: Bound<'_, PyAny> = node?;
            if !names.contains(native_field(&node, "name")?)? {
                continue;
            }
            let path: PathBuf = native_field(&node, "relative_path")?.extract()?;
            if let Some(folder) = path.parent() {
                folders.insert(format!("{TESTS_FOLDER}/{}/", folder.to_string_lossy()));
            }
        }
    }
    Ok(folders)
}

/// The project folder ahead of any inherited `PYTHONPATH`.
fn python_path(project_dir: &Path) -> PyResult<std::ffi::OsString> {
    let inherited: Vec<PathBuf> = std::env::var_os("PYTHONPATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default();
    std::env::join_paths(std::iter::once(project_dir.to_path_buf()).chain(inherited))
        .map_err(|error| PyOSError::new_err(format!("cannot build PYTHONPATH: {error}")))
}
