//! One assembly per test input, with the error its assembly raises, if any.

use crate::sql_test_glue::_helpers::assembly::AssemblyRequest;
use crate::sql_test_glue::_helpers::assembly::AssemblyRow;
use crate::sql_test_glue::_helpers::assembly::assemble_compiled_sql_tests;
use crate::sql_test_glue::_helpers::compiled_tests::Classes;
use crate::sql_test_glue::_helpers::compiled_tests::assembly;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// One assembly per test input, with the error its assembly raises, if any.
#[pyfunction]
#[pyo3(signature = (*, inputs, sql_analysis_dialect=None))]
pub fn assemble_native_sql_tests<'py>(
    py: Python<'py>,
    inputs: &Bound<'py, PyAny>,
    sql_analysis_dialect: Option<String>,
) -> PyResult<Bound<'py, PyTuple>> {
    let test_inputs: Vec<Bound<'py, PyAny>> = native_field(inputs, "test_inputs")?.extract()?;
    let request = AssemblyRequest {
        model_inputs: native_field(inputs, "model_inputs")?.extract()?,
        test_inputs: test_inputs.clone(),
        lexical_syntax: native_field(
            &native_field(inputs, "sql_lexical_syntax")?,
            "native_mapping",
        )?
        .extract()?,
        sql_analysis_dialect,
    };
    let rows: Vec<AssemblyRow> = assemble_compiled_sql_tests(py, request)?;
    if rows.len() != test_inputs.len() {
        return Err(compiler_error(
            "native SQL test assembly returned an invalid batch response",
        ));
    }
    let classes: Classes<'py> = Classes::new(py)?;
    let assemblies: Vec<Bound<'py, PyAny>> = test_inputs
        .iter()
        .zip(rows)
        .map(|(test_input, row)| assembly(&classes, test_input, row))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, assemblies)
}
