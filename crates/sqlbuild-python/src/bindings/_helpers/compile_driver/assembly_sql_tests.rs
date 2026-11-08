//! Every SQL test assembled natively; macro-mocked tests' model queries run the user's macros.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyList, PyTuple, PyTupleMethods};

use crate::bindings::_helpers::boundary::panics::NativeCompilerError;
use crate::bindings::_helpers::boundary::python_calls::{imported, keywords};
use crate::bindings::_helpers::compile_context::diagnostic_collector::report_compile_diagnostic;
use crate::bindings::_helpers::compile_driver::assembly_checks::test_model_query_overrides;
use crate::bindings::_helpers::compile_driver::stage_work::{SQL_TEST_ASSEMBLY, count_stage};
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use crate::bindings::_helpers::sql_test_glue::compiled_tests::assemble_native_sql_tests;

const DECIMAL_MODULE: &str = "decimal";
const DATACLASSES_MODULE: &str = "dataclasses";
const SQL_TEST_INPUT_FAILURE: &str = "input";
const SQL_TEST_DECIMAL_OVERFLOW: &str = "decimal_overflow";

/// Every compiled SQL test, raising each assembly failure in the tests' order.
pub(crate) fn assembled_sql_tests<'py>(
    inputs: &Bound<'py, PyAny>,
    sql_analysis_dialect: Option<String>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = inputs.py();
    count_stage(py, SQL_TEST_ASSEMBLY, 1)?;
    let assemblies: Vec<Bound<'py, PyAny>> =
        assemble_native_sql_tests(py, inputs, sql_analysis_dialect)?
            .iter()
            .collect();
    let test_inputs: Vec<Bound<'py, PyAny>> = native_field(inputs, "test_inputs")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    if assemblies.len() != test_inputs.len() {
        return Err(NativeCompilerError::new_err(
            "native SQL test assembly returned a different number of tests",
        ));
    }
    let mocked_input: Bound<'py, PyAny> =
        record_class(py, RecordId::CompileModelSqlTestInputPayload)?;
    let mut tests: Vec<Bound<'py, PyAny>> = Vec::with_capacity(assemblies.len());
    for (test_input, assembly) in test_inputs.iter().zip(assemblies) {
        let failure: Bound<'py, PyAny> = native_field(&assembly, "failure")?;
        let kind: Option<String> = if failure.is_none() {
            None
        } else {
            Some(native_field(&failure, "kind")?.extract()?)
        };
        let mut test: Bound<'py, PyAny> = native_field(&assembly, "test")?;
        if test.is_none() {
            return Err(assembly_error(py, &failure, kind.as_deref())?);
        }
        for item in native_field(&assembly, "diagnostics")?.try_iter()? {
            let (key, diagnostic): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
            report_compile_diagnostic(&key, &diagnostic)?;
        }
        let payload: Bound<'py, PyAny> = native_field(test_input, "payload")?;
        if payload.is_instance(&mocked_input)?
            && native_field(&payload, "macro_mocks")?.is_truthy()?
        {
            test = with_model_queries(
                &test,
                test_model_query_overrides(test_input, inputs)?.as_any(),
            )?;
        }
        if kind.as_deref() == Some(SQL_TEST_DECIMAL_OVERFLOW) {
            let overflow: Bound<'py, PyAny> = imported(py, DECIMAL_MODULE, "Overflow")?;
            let raised: Bound<'py, PyAny> = overflow.call1((PyList::new(py, [&overflow])?,))?;
            return Err(PyErr::from_value(raised));
        }
        tests.push(test);
    }
    PyTuple::new(py, tests)
}

/// The failure of a test that did not assemble: input errors as `CompileInputError`.
fn assembly_error(
    py: Python<'_>,
    failure: &Bound<'_, PyAny>,
    kind: Option<&str>,
) -> PyResult<PyErr> {
    let message: String = if failure.is_none() {
        String::new()
    } else {
        native_field(failure, "message")?.extract()?
    };
    if kind == Some(SQL_TEST_INPUT_FAILURE) {
        let error: Bound<'_, PyAny> = error_class(py, ErrorId::CompileInput)?.call1((message,))?;
        return Ok(PyErr::from_value(error));
    }
    Ok(NativeCompilerError::new_err(message))
}

/// `test` with its model payload's query overrides set to `queries`.
fn with_model_queries<'py>(
    test: &Bound<'py, PyAny>,
    queries: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = test.py();
    let payload: Bound<'py, PyAny> = native_field(test, "payload")?;
    if !payload.is_instance(&record_class(py, RecordId::CompiledModelSqlTestPayload)?)? {
        return Ok(test.clone());
    }
    let replace: Bound<'py, PyAny> = imported(py, DATACLASSES_MODULE, "replace")?;
    let payload: Bound<'py, PyAny> = replace.call(
        (payload,),
        Some(&keywords(
            py,
            &[("model_query_overrides", queries.clone())],
        )?),
    )?;
    replace.call((test,), Some(&keywords(py, &[("payload", payload)])?))
}
