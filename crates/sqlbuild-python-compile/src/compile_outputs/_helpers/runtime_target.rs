//! SQL test artifact paths, and the executed SQL and Python check results written to `target/run`.

use crate::compile_outputs::main::write_runtime_target::write_runtime_target;
use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python, pyfunction};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::target_paths::models::SqlTestPathFacts;
use sqlbuild_host::models::BuildExecutionResult;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::types::ReadResult;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) const RUN_DIR: &str = "run";
pub(crate) const TESTS_DIR: &str = "tests";
pub(crate) const CHECKS_DIR: &str = "checks";
pub(crate) const PYTHON_CHECKS_FILE: &str = "python_checks.json";
pub(crate) const PYTHON_CHECK_KIND: &str = "python_check";
pub(crate) const JSON_INDENT: i64 = 2;

/// One test's identity as the test runtime target matches entries to results.
#[derive(Hash, PartialEq, Eq)]
pub(crate) struct TestKey {
    pub(crate) name: String,
    pub(crate) source_path: Option<String>,
    pub(crate) block_index: Option<i64>,
    pub(crate) case_name: Option<String>,
}

fn posix(path: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if path.is_none() {
        return Ok(None);
    }
    path.call_method0("as_posix")?.extract().map(Some)
}

pub(crate) fn path_object<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    imported(py, "pathlib", "Path")?.call1((text,))
}

pub(crate) fn test_key(value: &Bound<'_, PyAny>, name_field: &str) -> PyResult<TestKey> {
    Ok(TestKey {
        name: native_field(value, name_field)?.extract()?,
        source_path: posix(&native_field(value, "source_path")?)?,
        block_index: native_field(value, "block_index")?.extract()?,
        case_name: native_field(value, "case_name")?.extract()?,
    })
}

pub(crate) fn path_facts(
    test: &Bound<'_, PyAny>,
    model_names: Vec<String>,
) -> PyResult<SqlTestPathFacts> {
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

pub(crate) fn write_text(path: &Bound<'_, PyAny>, text: &str) -> PyResult<()> {
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

pub(crate) fn entry_paths<T>(
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

pub(crate) fn relative_posix(entry: &Bound<'_, PyAny>) -> PyResult<String> {
    native_field(entry, "relative_path")?
        .call_method0("as_posix")?
        .extract()
}

pub(crate) fn check_status(result: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    if native_field(result, "passed")?.is_truthy()? {
        return Ok("pass");
    }
    if native_field(result, "warned")?.is_truthy()? {
        return Ok("warn");
    }
    Ok("fail")
}

/// Write executed model and function lifecycle SQL under `target/run`.
#[pyfunction(name = "write_runtime_target")]
#[pyo3(signature = (*, target_dir, plan_output, result))]
pub(crate) fn write_runtime_target_binding(
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
