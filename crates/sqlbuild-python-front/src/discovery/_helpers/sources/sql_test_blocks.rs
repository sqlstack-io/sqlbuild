//! SQL `TEST(...)` blocks validated natively and built as discovery records.

use std::collections::HashMap;
use std::sync::Mutex;

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_host::entries::models::FacadeHost;
use sqlbuild_model_config::sql_tests::main::sql_test_block::sql_test_block;
use sqlbuild_model_config::sql_tests::main::sql_test_names_failure::sql_test_names_failure;
use sqlbuild_model_config::sql_tests::models::{SqlTestBlock, SqlTestCase};
use sqlbuild_model_config::sql_values::models::{RawKey, RawSqlValue};

use crate::authored_values::main::raw_value::raw_value;
use crate::authored_values::main::sql_value::sql_value;
use crate::authored_values::models::SqlValueClasses;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

pub(crate) type NativeBlock<'py> = (Bound<'py, PyDict>, String, Bound<'py, PyAny>);

fn parse_error(py: Python<'_>, message: String) -> PyResult<PyErr> {
    Ok(PyErr::from_value(
        error_class(py, ErrorId::SqlTestParse)?.call1((message,))?,
    ))
}

fn header_entries(
    header: &Bound<'_, PyDict>,
    classes: &SqlValueClasses<'_>,
) -> PyResult<Vec<(RawKey, RawSqlValue)>> {
    match raw_value(header.as_any(), classes)? {
        RawSqlValue::Object(entries) => Ok(entries),
        _ => Ok(Vec::new()),
    }
}

fn case_record<'py>(
    py: Python<'py>,
    (case_index, case): (usize, &SqlTestCase),
    classes: &SqlValueClasses<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut values: Vec<Bound<'py, PyTuple>> = Vec::with_capacity(case.values.len());
    for (name, value) in &case.values {
        values.push(PyTuple::new(
            py,
            [name.into_bound_py_any(py)?, sql_value(value, classes)?],
        )?);
    }
    new_record(
        py,
        RecordId::DiscoveredSqlTestCase,
        (case.name.as_str(), PyTuple::new(py, values)?, case_index),
        None,
    )
}

/// `SqlTestMode(mode)`, the member each mode resolves to looked up once per process.
fn test_mode<'py>(py: Python<'py>, mode: &'static str) -> PyResult<Bound<'py, PyAny>> {
    static MODES: PyOnceLock<Mutex<HashMap<&'static str, Py<PyAny>>>> = PyOnceLock::new();
    let modes: &Mutex<HashMap<&'static str, Py<PyAny>>> =
        MODES.get_or_init(py, || Mutex::new(HashMap::new()));
    let mut known = modes
        .lock()
        .map_err(|_| PyRuntimeError::new_err("SQL test mode cache is poisoned"))?;
    if let Some(member) = known.get(mode) {
        return Ok(member.bind(py).clone());
    }
    let member: Bound<'py, PyAny> = host_entry(py, FacadeHost::SqlTestMode)?.call1((mode,))?;
    let _ = known.insert(mode, member.clone().unbind());
    Ok(member)
}

fn block_record<'py>(
    py: Python<'py>,
    (index, native): (usize, &NativeBlock<'py>),
    block: SqlTestBlock,
    classes: &SqlValueClasses<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let parameters = block
        .parameters
        .iter()
        .map(|parameter| {
            new_record(
                py,
                RecordId::SqlTestParameterDeclaration,
                (
                    parameter.name.as_str(),
                    enum_from_value(py, RecordId::SqlValueKind, parameter.value_type)?,
                    parameter.nullable,
                ),
                None,
            )
        })
        .collect::<PyResult<Vec<_>>>()?;
    let cases = block
        .cases
        .iter()
        .enumerate()
        .map(|(case_index, case)| case_record(py, (case_index, case), classes))
        .collect::<PyResult<Vec<_>>>()?;
    let keywords = PyDict::new(py);
    keywords.set_item("test_index", index)?;
    keywords.set_item("header_values", &native.0)?;
    keywords.set_item("sql_body", native.1.as_str())?;
    keywords.set_item("name", block.name)?;
    keywords.set_item("mode", test_mode(py, block.mode)?)?;
    keywords.set_item("parameters", PyTuple::new(py, parameters)?)?;
    keywords.set_item("cases", PyTuple::new(py, cases)?)?;
    keywords.set_item("cursor_start", block.cursor_start)?;
    keywords.set_item("cursor_end", block.cursor_end)?;
    keywords.set_item("sql_body_span", &native.2)?;
    new_record(py, RecordId::DiscoveredSqlTestBlock, (), Some(&keywords))
}

/// Each `(header, sql_body, span)` block of a SQL test file as a `DiscoveredSqlTestBlock`.
pub(crate) fn sql_test_block_records<'py>(
    py: Python<'py>,
    file_path: &str,
    blocks: Vec<NativeBlock<'py>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let classes = SqlValueClasses::load(py)?;
    let mut records: Vec<Bound<'py, PyAny>> = Vec::with_capacity(blocks.len());
    for (offset, native) in blocks.iter().enumerate() {
        let header = header_entries(&native.0, &classes)?;
        match sql_test_block(&header, &native.1, file_path) {
            Ok(block) => records.push(block_record(py, (offset + 1, native), block, &classes)?),
            Err(message) => return Err(parse_error(py, message)?),
        }
    }
    Ok(records)
}

/// `validate_sql_test_names` over each block's `(test_index, name)`.
pub(crate) fn validate_block_names(
    py: Python<'_>,
    file_path: &str,
    blocks: &[(usize, Option<String>)],
) -> Result<(), PyErr> {
    match sql_test_names_failure(file_path, blocks) {
        None => Ok(()),
        Some(message) => Err(parse_error(py, message)?),
    }
}
