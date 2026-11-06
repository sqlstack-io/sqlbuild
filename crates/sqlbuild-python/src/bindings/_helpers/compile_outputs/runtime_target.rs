//! SQL test artifact paths, and the executed SQL and Python check results written to `target/run`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python, pyfunction,
};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
use pyo3::wrap_pyfunction;
use sqlbuild_analysis::target_paths::main::function_run_path::function_run_path;
use sqlbuild_analysis::target_paths::main::model_run_path::model_run_path;
use sqlbuild_analysis::target_paths::main::runtime_sql_text::runtime_sql_text;
use sqlbuild_analysis::target_paths::main::sql_test_output_path::sql_test_output_path;
use sqlbuild_analysis::target_paths::models::SqlTestPathFacts;
use sqlbuild_core::text::main::python_isspace::python_isspace;
use sqlbuild_host::models::{
    BuildExecutionResult, LifeCycleEvent, LifeCycleEventKind, SqlTestExecutionResult,
};

use crate::bindings::_helpers::analysis::sql_test_comparisons::single_comparison_sql;
use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::native_host::results::ReadResult;
use crate::bindings::_helpers::records::fields::field as native_field;

const RUN_DIR: &str = "run";
const TESTS_DIR: &str = "tests";
const CHECKS_DIR: &str = "checks";
const PYTHON_CHECKS_FILE: &str = "python_checks.json";
const PYTHON_CHECK_KIND: &str = "python_check";
const JSON_INDENT: i64 = 2;

/// One test's identity as the test runtime target matches entries to results.
#[derive(Hash, PartialEq, Eq)]
struct TestKey {
    name: String,
    source_path: Option<String>,
    block_index: Option<i64>,
    case_name: Option<String>,
}

fn posix(path: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if path.is_none() {
        return Ok(None);
    }
    path.call_method0("as_posix")?.extract().map(Some)
}

fn path_object<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    imported(py, "pathlib", "Path")?.call1((text,))
}

fn test_key(value: &Bound<'_, PyAny>, name_field: &str) -> PyResult<TestKey> {
    Ok(TestKey {
        name: native_field(value, name_field)?.extract()?,
        source_path: posix(&native_field(value, "source_path")?)?,
        block_index: native_field(value, "block_index")?.extract()?,
        case_name: native_field(value, "case_name")?.extract()?,
    })
}

fn path_facts(test: &Bound<'_, PyAny>, model_names: Vec<String>) -> PyResult<SqlTestPathFacts> {
    let TestKey {
        name,
        source_path,
        block_index,
        case_name,
    } = test_key(test, "name")?;
    Ok(SqlTestPathFacts {
        name,
        source_path,
        block_index,
        case_name,
        model_names,
    })
}

/// The target path of one planned SQL test, relative to its artifact directory.
pub(crate) fn planned_sql_test_path<'py>(entry: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let mut model_names: Vec<String> = Vec::new();
    for step in native_field(entry, "chain")?.try_iter()? {
        model_names.push(native_field(&step?, "model_name")?.extract()?);
    }
    path_object(
        entry.py(),
        &sql_test_output_path(&path_facts(entry, model_names)?),
    )
}

/// The target path of one compiled SQL test from its chain's model names.
pub(crate) fn compiled_sql_test_path(
    test: &Bound<'_, PyAny>,
    model_names: Vec<String>,
) -> PyResult<PathBuf> {
    Ok(PathBuf::from(sql_test_output_path(&path_facts(
        test,
        model_names,
    )?)))
}

fn write_text(path: &Bound<'_, PyAny>, text: &str) -> PyResult<()> {
    let py: Python<'_> = path.py();
    let mkdir: Bound<'_, PyDict> = PyDict::new(py);
    mkdir.set_item("parents", true)?;
    mkdir.set_item("exist_ok", true)?;
    native_field(path, "parent")?.call_method("mkdir", (), Some(&mkdir))?;
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("encoding", "utf-8")?;
    path.call_method("write_text", (text,), Some(&options))?;
    Ok(())
}

fn entry_paths<T>(
    entries: &Bound<'_, PyAny>,
    path_of: impl Fn(&Bound<'_, PyAny>) -> PyResult<T>,
) -> PyResult<HashMap<String, T>> {
    let mut paths: HashMap<String, T> = HashMap::new();
    for entry in entries.try_iter()? {
        let entry: Bound<'_, PyAny> = entry?;
        paths.insert(native_field(&entry, "name")?.extract()?, path_of(&entry)?);
    }
    Ok(paths)
}

fn relative_posix(entry: &Bound<'_, PyAny>) -> PyResult<String> {
    native_field(entry, "relative_path")?
        .call_method0("as_posix")?
        .extract()
}

/// Write executed model and function lifecycle SQL under `target/run`.
pub(crate) fn write_runtime_target(
    target_dir: &Bound<'_, PyAny>,
    plan: &Bound<'_, PyAny>,
    result: &BuildExecutionResult<Bound<'_, PyAny>>,
) -> PyResult<()> {
    let run_dir: Bound<'_, PyAny> = target_dir.div(RUN_DIR)?;
    let models: HashMap<String, String> =
        entry_paths(&native_field(plan, "model_entries")?, |entry| {
            relative_posix(entry).map(|path| model_run_path(&path))
        })?;
    let functions: HashMap<String, String> =
        entry_paths(&native_field(plan, "function_entries")?, |entry| {
            Ok(function_run_path(
                &relative_posix(entry)?,
                &native_field(entry, "language")?
                    .getattr("value")?
                    .extract::<String>()?,
            ))
        })?;
    let models = result.model_results.iter().map(|model| {
        (
            models.get(&model.model_name),
            model.lifecycle_events.as_slice(),
        )
    });
    let functions = result.function_results.iter().map(|function| {
        (
            functions.get(&function.function_name),
            function.lifecycle_events.as_slice(),
        )
    });
    for (path, events) in models.chain(functions) {
        let statements: Vec<String> = events
            .iter()
            .filter(|event: &&LifeCycleEvent| event.kind == LifeCycleEventKind::Sql)
            .map(|event| event.content.clone())
            .collect();
        if let Some(path) = path.filter(|_| !statements.is_empty()) {
            write_text(&run_dir.div(path)?, &runtime_sql_text(&statements))?;
        }
    }
    Ok(())
}

/// Write the executed SQL unit-test comparison statements under `target/run/tests`.
pub(crate) fn write_test_runtime_target(
    (target_dir, adapter): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    plan: &Bound<'_, PyAny>,
    results: &[SqlTestExecutionResult<Bound<'_, PyAny>>],
) -> PyResult<()> {
    let mut executed: HashSet<TestKey> = HashSet::new();
    for result in results {
        executed.insert(TestKey {
            name: result.test_name.clone(),
            source_path: result.source_path.as_ref().map(|path| {
                path.to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/")
            }),
            block_index: result.block_index,
            case_name: result.case_name.clone(),
        });
    }
    let tests_dir: Bound<'_, PyAny> = target_dir.div(RUN_DIR)?.div(TESTS_DIR)?;
    for entry in native_field(plan, "test_entries")?.try_iter()? {
        let entry: Bound<'_, PyAny> = entry?;
        if !executed.contains(&test_key(&entry, "name")?) {
            continue;
        }
        let sql: String = single_comparison_sql(&entry, adapter)?;
        write_text(
            &tests_dir.div(planned_sql_test_path(&entry)?)?,
            &format!("{}\n", sql.trim_end_matches(python_isspace)),
        )?;
    }
    Ok(())
}

fn check_status(result: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    if native_field(result, "passed")?.is_truthy()? {
        return Ok("pass");
    }
    if native_field(result, "warned")?.is_truthy()? {
        return Ok("warn");
    }
    Ok("fail")
}

/// Write Python check results under `target/run/checks` when any ran.
pub(crate) fn write_python_check_runtime_target(
    target_dir: &Bound<'_, PyAny>,
    results: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = target_dir.py();
    let checks: Bound<'_, PyList> = PyList::empty(py);
    for result in results.try_iter()? {
        let result: Bound<'_, PyAny> = result?;
        let name: Bound<'_, PyAny> = native_field(&result, "node_name")?;
        let check: Bound<'_, PyDict> = PyDict::new(py);
        check.set_item("kind", PYTHON_CHECK_KIND)?;
        check.set_item("name", &name)?;
        check.set_item("display_name", &name)?;
        check.set_item("check_id", format!("{PYTHON_CHECK_KIND}:{}", name.str()?))?;
        check.set_item("status", check_status(&result)?)?;
        check.set_item("passed", native_field(&result, "passed")?)?;
        check.set_item(
            "severity",
            native_field(&result, "severity")?.getattr("value")?,
        )?;
        check.set_item("message", native_field(&result, "message")?)?;
        check.set_item("error_message", native_field(&result, "error_message")?)?;
        check.set_item("metadata", native_field(&result, "metadata")?)?;
        checks.append(check)?;
    }
    if checks.is_empty() {
        return Ok(());
    }
    let payload: Bound<'_, PyDict> = PyDict::new(py);
    payload.set_item("checks", checks)?;
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("indent", JSON_INDENT)?;
    let text: String = imported(py, "json", "dumps")?
        .call((payload,), Some(&options))?
        .extract()?;
    write_text(
        &target_dir
            .div(RUN_DIR)?
            .div(CHECKS_DIR)?
            .div(PYTHON_CHECKS_FILE)?,
        &format!("{text}\n"),
    )
}

/// Write executed model and function lifecycle SQL under `target/run`.
#[pyfunction(name = "write_runtime_target")]
#[pyo3(signature = (*, target_dir, plan_output, result))]
fn write_runtime_target_binding(
    target_dir: &Bound<'_, PyAny>,
    plan_output: &Bound<'_, PyAny>,
    result: &Bound<'_, PyAny>,
) -> PyResult<()> {
    write_runtime_target(
        target_dir,
        plan_output,
        &BuildExecutionResult::read(result)?,
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(write_runtime_target_binding, module)?)
}
