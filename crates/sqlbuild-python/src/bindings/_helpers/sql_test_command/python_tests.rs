//! The project's Python tests, which a local `sqb test` runs with pytest after its SQL tests.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::Instant;

use pyo3::exceptions::{PyOSError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyFrozenSetMethods;
use serde_json::{Value, json};
use sqlbuild_core::cli_text::main::format_phase_line::format_phase_line;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_discovery::python_paths::main::python_test_paths::python_test_paths;

use sqlbuild_python_base::build_progress::main::terminal::flush::flush;
use sqlbuild_python_base::build_progress::main::terminal::write::write;
use sqlbuild_python_base::native_host::main::flush_console::flush_console;
use sqlbuild_python_base::native_host::main::module_installed::module_installed;
use sqlbuild_python_base::native_host::main::python_executable::python_executable;
use sqlbuild_python_base::records::main::construct::new_error::new_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_compile::compile_driver::main::setup::project_graph::project_graph;
use sqlbuild_python_project::graph::main::python_nodes::python_node_graph::python_node_graph;
use sqlbuild_python_project::graph::main::python_nodes::python_node_names::python_node_names;
use sqlbuild_python_project::graph::main::python_nodes::run_selection_from_inputs::run_selection_from_inputs;
use sqlbuild_python_project::planner_selection::main::split_project_unit_test_selectors::split_project_unit_test_selectors;

/// The prefix of a Python test module's file name.
const TEST_MODULE_PREFIX: &str = "test_";
const PYTEST_MODULE: &str = "pytest";
/// The diagnostic code of a Python test run that cannot start.
pub(crate) const PYTHON_TESTS_INPUT_CODE: &str = "S000";
/// The project folder whose `python/`, `providers/` and `orchestration/` folders mirror the project's.
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
/// The JSON section reporting the Python tests a `sqb test` ran.
const PYTHON_TESTS_SECTION: &str = "python_tests";
const PASSED_STATUS: &str = "success";
const FAILED_STATUS: &str = "failed";

/// Whether the request selects or excludes anything.
pub(crate) fn selective(request: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(native_field(request, "select")?.is_truthy()?
        || native_field(request, "exclude")?.is_truthy()?)
}

/// Every `test_*.py` module under the project's `tests/python/`, `tests/providers/` and `tests/orchestration/`.
pub(crate) fn python_test_modules(invocation: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let project_dir: PathBuf = native_field(invocation, "effective_project_dir")?.extract()?;
    Ok(python_test_paths(&project_dir)
        .into_iter()
        .filter(|parts| {
            parts
                .last()
                .is_some_and(|name| name.starts_with(TEST_MODULE_PREFIX))
        })
        .map(|parts| parts.join("/"))
        .collect())
}

/// Whether a selector of the request reaches a discovered Python node, such as `+storefront_import` or `tag:ingest`.
pub(crate) fn names_python_node(
    request: &Bound<'_, PyAny>,
    discovered: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let graph: Bound<'_, PyAny> = python_node_graph(discovered)?;
    let mut selectors: Vec<String> = native_field(request, "select")?.extract()?;
    selectors.extend(native_field(request, "exclude")?.extract::<Vec<String>>()?);
    for selector in selectors {
        if python_node_names(&graph, (vec![selector], Vec::new()))
            .is_ok_and(|names| !PyFrozenSetMethods::is_empty(&names))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The Python test `modules` a local `sqb test` runs: all, or those mirroring the selected Python nodes.
pub(crate) fn selected_python_tests(
    request: &Bound<'_, PyAny>,
    invocation: &Bound<'_, PyAny>,
    (pipeline, modules): (Option<&Bound<'_, PyAny>>, Vec<String>),
) -> PyResult<Vec<String>> {
    let py: Python<'_> = request.py();
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
        return Err(new_error(
            py,
            ErrorId::PlannerInput,
            "no Python tests found",
            (
                Some(PYTHON_TESTS_INPUT_CODE),
                Some(
                    "add test_*.py modules under tests/python/, tests/providers/ or tests/orchestration/, mirroring the code they test",
                ),
            ),
        ));
    }
    if !selected.is_empty() && !module_installed(py, PYTEST_MODULE)? {
        return Err(new_error(
            py,
            ErrorId::PlannerInput,
            "Python tests found but pytest isn't installed",
            (
                Some(PYTHON_TESTS_INPUT_CODE),
                Some("install the dev dependencies, or run `sqb test --sql`"),
            ),
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
    let stream: Bound<'_, PyAny> = native_field(invocation, "progress_stream")?;
    let style: CliStyle = CliStyle {
        use_color: native_field(invocation, "use_color")?.is_truthy()?,
    };
    let project_dir: PathBuf = native_field(invocation, "effective_project_dir")?.extract()?;
    let interpreter: PathBuf = python_executable(py)?;
    let files: String = match modules.len() {
        1 => "1 file".to_owned(),
        count => format!("{count} files"),
    };
    write(
        &stream,
        &format!("\nRunning Python tests with pytest  ({files})\n"),
    )?;
    flush(&stream)?;
    flush_console(py, "stdout")?;
    let mut command: Command = Command::new(&interpreter);
    command
        .args(["-m", PYTEST_MODULE, IMPORT_MODE])
        .args(modules)
        .current_dir(&project_dir)
        .env("PYTHONPATH", python_path(&project_dir)?)
        .stdin(Stdio::null());
    if native_field(request, "json_output")?.is_truthy()? {
        command.stdout(Stdio::from(std::io::stderr()));
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
    write(&stream, &format!("{line}\n"))?;
    flush(&stream)?;
    Ok(code)
}

/// The `tests/` folders mirroring the Python nodes the request's resource selectors resolve to.
fn mirrored_folders(
    request: &Bound<'_, PyAny>,
    pipeline: &Bound<'_, PyAny>,
    discovered: &Bound<'_, PyAny>,
) -> PyResult<BTreeSet<String>> {
    let split: Bound<'_, PyAny> = split_project_unit_test_selectors(
        &native_field(request, "select")?,
        &native_field(request, "exclude")?,
        discovered,
        true,
    )?;
    let select: Bound<'_, PyAny> = native_field(&split, "select")?;
    let exclude: Bound<'_, PyAny> = native_field(&split, "exclude")?;
    if !select.is_truthy()? && native_field(request, "select")?.is_truthy()? {
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

/// `payload` with a `python_tests` section for `modules` and pytest's exit `code`; a failure fails it.
pub(crate) fn with_python_section(
    payload: &str,
    modules: &[String],
    code: i32,
) -> PyResult<String> {
    let invalid = |error: serde_json::Error| PyValueError::new_err(error.to_string());
    let mut document: Value = serde_json::from_str(payload).map_err(invalid)?;
    let status: &str = if code == 0 {
        PASSED_STATUS
    } else {
        FAILED_STATUS
    };
    if let Value::Object(fields) = &mut document {
        if code != 0 {
            fields.insert("status".to_owned(), json!(FAILED_STATUS));
        }
        fields.insert(
            PYTHON_TESTS_SECTION.to_owned(),
            json!({ "status": status, "exit_code": code, "files": modules }),
        );
    }
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&document).map_err(invalid)?
    ))
}
