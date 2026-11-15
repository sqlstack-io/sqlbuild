//! Project native planning failures into existing located diagnostic exceptions.

use crate::bindings::_helpers::sql_test_glue::plan_values::string;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::sql_test_attachment::main::sql_test_cte_location::sql_test_cte_location;
use sqlbuild_analysis::sql_test_attachment::models::CteLocation;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

/// The location of `call` inside one CTE of a SQL test block, else the CTE header, else the block.
fn cte_call_location<'py>(
    py: Python<'py>,
    test: &Bound<'py, PyAny>,
    (cte, call): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let test_file: Bound<'py, PyAny> = native_field(test, "test_file")?;
    let call: Option<String> = call.extract()?;
    let located: CteLocation = sql_test_cte_location(
        &native_field(&test_file, "contents")?.extract::<String>()?,
        &native_field(&native_field(test, "test_block")?, "sql_body")?.extract::<String>()?,
        &cte.extract::<String>()?,
        call.as_deref(),
    );
    let arguments: Bound<'py, PyDict> = PyDict::new(py);
    arguments.set_item("path", native_field(&test_file, "relative_path")?)?;
    arguments.set_item("line", located.line)?;
    arguments.set_item("column", located.column)?;
    arguments.set_item("end_line", located.end_line)?;
    arguments.set_item("end_column", located.end_column)?;
    new_record(py, RecordId::SourceLocation, (), Some(&arguments))
}

pub(super) fn planning_error(
    py: Python<'_>,
    error: PyErr,
    tests: &Bound<'_, PyAny>,
) -> PyResult<PyErr> {
    let message = error.value(py).str()?.to_string();
    if let Some(payload) = message.strip_prefix("sql_test_reference:") {
        let payload = imported(py, "orjson", "loads")?.call1((payload,))?;
        let mut found = None;
        for test in tests.try_iter()? {
            let test = test?;
            if native_field(&test, "name")?.eq(payload.get_item("testName")?)?
                && native_field(&native_field(&test, "test_file")?, "relative_path")?
                    .str()?
                    .into_any()
                    .eq(payload.get_item("fileLabel")?)?
            {
                found = Some(test);
                break;
            }
        }
        let cte = payload.get_item("cteName")?;
        let call = payload.get_item("call")?;
        let message = format!(
            "SQL test CTE '{}' calls {}, which the test query cannot resolve",
            cte.extract::<String>()?,
            call.extract::<String>()?
        );
        let help = format!(
            "Mock it in the test, for example {} AS (SELECT ...), or call __ref(\"<model>\") on a model the test runs.",
            payload.get_item("mockCte")?.extract::<String>()?
        );
        let kwargs = PyDict::new(py);
        kwargs.set_item("help", help)?;
        let class = if let Some(test) = found {
            kwargs.set_item("location", cte_call_location(py, &test, (&cte, &call))?)?;
            ErrorId::SqlTestReference
        } else {
            kwargs.set_item("code", super::constants::SQL_TEST_HELPER_REFERENCE_CODE)?;
            ErrorId::CompileInput
        };
        return Ok(PyErr::from_value(
            error_class(py, class)?.call((message,), Some(&kwargs))?,
        ));
    }
    let (class, message) = if let Some(message) = message.strip_prefix("compile_input:") {
        (ErrorId::CompileInput, message.to_owned())
    } else if let Some(message) = message.strip_prefix("planner_input:") {
        (ErrorId::PlannerInput, message.to_owned())
    } else {
        (
            ErrorId::NativeSqlTestPlanning,
            format!("native SQL-test planning failed: {message}"),
        )
    };
    Ok(PyErr::from_value(
        error_class(py, class)?.call1((string(py, &message),))?,
    ))
}
