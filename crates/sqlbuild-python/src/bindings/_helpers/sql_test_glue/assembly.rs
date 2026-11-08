//! Assemble compiled SQL test facts from compile inputs' own objects, without a JSON request.

use pyo3::PyErr;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::compiler::main::sql_test_assembly::assemble_sql_test_batch;
use sqlbuild_analysis::compiler::models::{
    AssembledSqlTestFacts, SqlTestAssemblyBatch, SqlTestAssemblyFailure, SqlTestAssemblyModel,
    SqlTestAssemblyModelPayload, SqlTestAssemblyOutcome, SqlTestAssemblyPayload,
    SqlTestAssemblyReference, SqlTestAssemblyTest, SqlTestCte, SqlTestParameterValue,
};

use sqlbuild_core::panics::main::native_failure::native_failure;

use crate::bindings::_helpers::boundary::panics::value_error;
use crate::bindings::_helpers::compiled_views::model_input_view::NativeModelInput;
use crate::bindings::_helpers::records::fields::{
    field as native_field, path_posix_text, path_stem,
};
use crate::bindings::_helpers::sqltext::lexical_syntax::LexicalSyntaxInput;
use crate::bindings::types::CompilerDetach;
use sqlbuild_project::models::ReferenceRow;

const MACRO_TOKEN: char = '@';
const INPUT_FAILURE: &str = "input";
const INTERNAL_FAILURE: &str = "internal";
const DECIMAL_OVERFLOW: &str = "decimal_overflow";
const SQL_TEST_ASSEMBLY_CONTEXT: &str = "native SQL test assembly";

/// The test's facts, and the `(kind, message)` of the error its assembly raises, if any.
pub(crate) type AssemblyRow = (
    Option<AssembledSqlTestFacts>,
    Option<(&'static str, String)>,
);

/// The compile inputs assembly reads: model inputs, SQL test inputs and lexical rules.
pub(crate) struct AssemblyRequest<'py> {
    pub(crate) model_inputs: Vec<Bound<'py, PyAny>>,
    pub(crate) test_inputs: Vec<Bound<'py, PyAny>>,
    pub(crate) lexical_syntax: LexicalSyntaxInput,
    pub(crate) sql_analysis_dialect: Option<String>,
}

/// Each test input's assembled facts, or the error its assembly raises.
pub(crate) fn assemble_compiled_sql_tests(
    py: Python<'_>,
    request: AssemblyRequest<'_>,
) -> PyResult<Vec<AssemblyRow>> {
    let batch = SqlTestAssemblyBatch {
        models: request
            .model_inputs
            .iter()
            .map(assembly_model)
            .collect::<PyResult<_>>()?,
        tests: request
            .test_inputs
            .iter()
            .map(assembly_test)
            .collect::<PyResult<_>>()?,
        lexical_syntax: request.lexical_syntax.into(),
        sql_analysis_dialect: request.sql_analysis_dialect,
    };
    let outcomes = py
        .compiler_detach(|| Ok(assemble_sql_test_batch(&batch)))
        .map_err(value_error)?;
    Ok(outcomes.into_iter().map(assembly_row).collect())
}

fn assembly_model(model: &Bound<'_, PyAny>) -> PyResult<SqlTestAssemblyModel> {
    if let Ok(native) = model.cast::<NativeModelInput>() {
        return native.get().read(model.py(), |record, _| {
            let unscanned_macro_source = if record.macro_deps.is_empty() {
                Some(record.macro_source_sql.to_string()).filter(|sql| sql.contains(MACRO_TOKEN))
            } else {
                None
            };
            Ok(SqlTestAssemblyModel {
                name: record.name.clone(),
                macro_deps: record.macro_deps.clone(),
                unscanned_macro_source,
                references: record
                    .references
                    .iter()
                    .map(
                        |ReferenceRow {
                             kind,
                             name,
                             package,
                             ..
                         }| SqlTestAssemblyReference {
                            kind: kind.clone(),
                            name: name.clone(),
                            package: package.clone(),
                        },
                    )
                    .collect(),
            })
        });
    }
    let macro_deps: Vec<String> = native_field(model, "macro_deps")?.extract()?;
    let unscanned_macro_source = if macro_deps.is_empty() {
        Some(native_field(model, "macro_source_sql")?.extract::<String>()?)
            .filter(|sql| sql.contains(MACRO_TOKEN))
    } else {
        None
    };
    Ok(SqlTestAssemblyModel {
        name: native_field(
            &native_field(&native_field(model, "model_file")?, "file_path")?,
            "stem",
        )?
        .extract()?,
        macro_deps,
        unscanned_macro_source,
        references: native_field(model, "references")?
            .try_iter()?
            .map(|reference| {
                let reference = reference?;
                Ok(SqlTestAssemblyReference {
                    kind: native_field(&reference, "ref_kind")?.extract()?,
                    name: native_field(&reference, "ref_name")?.extract()?,
                    package: native_field(&reference, "ref_package")?.extract()?,
                })
            })
            .collect::<PyResult<_>>()?,
    })
}

fn assembly_test(test: &Bound<'_, PyAny>) -> PyResult<SqlTestAssemblyTest> {
    let test_file = native_field(test, "test_file")?;
    let test_block = native_field(test, "test_block")?;
    let relative_path = native_field(&test_file, "relative_path")?;
    let payload = native_field(test, "payload")?;
    let payload = if payload.hasattr("actual_cte")? {
        SqlTestAssemblyPayload::Direct {
            mode: native_field(&payload, "mode")?.extract()?,
            tested_resource_names: native_field(&payload, "tested_resource_names")?.extract()?,
        }
    } else {
        SqlTestAssemblyPayload::Model(SqlTestAssemblyModelPayload {
            authored_ctes: ctes(&payload, "authored_ctes")?,
            expected_ctes: ctes(&payload, "expected_ctes")?,
            assertion_ctes: ctes(&payload, "assertion_ctes")?,
            expected_model_names: native_field(&payload, "expected_model_names")?.extract()?,
            assertion_target_model_names: native_field(&payload, "assertion_target_model_names")?
                .extract()?,
            reference_target_model_names: native_field(&payload, "reference_target_model_names")?
                .extract()?,
            mock_model_names: native_field(&payload, "mock_model_names")?.extract()?,
        })
    };
    Ok(SqlTestAssemblyTest {
        block_name: native_field(&test_block, "name")?.extract()?,
        file_stem: path_stem(&native_field(&test_file, "file_path")?)?,
        relative_path: path_posix_text(&relative_path)?,
        relative_stem: path_stem(&relative_path)?,
        contents: native_field(&test_file, "contents")?.extract()?,
        block_sql: native_field(&test_block, "sql_body")?.extract()?,
        block_index: native_field(&test_block, "test_index")?.extract()?,
        sql_body: native_field(test, "sql_body")?.extract()?,
        case_name: native_field(test, "case_name")?.extract()?,
        parameter_schema: native_field(test, "parameter_schema")?
            .try_iter()?
            .map(|parameter| {
                let parameter = parameter?;
                Ok((
                    native_field(&parameter, "name")?.extract()?,
                    native_field(&parameter, "value_type")?.extract()?,
                    native_field(&parameter, "nullable")?.extract()?,
                ))
            })
            .collect::<PyResult<_>>()?,
        parameter_values: native_field(test, "parameter_values")?
            .try_iter()?
            .map(|entry| {
                let (name, value): (String, Bound<'_, PyAny>) = entry?.extract()?;
                Ok((name, parameter_value(&value)?))
            })
            .collect::<PyResult<_>>()?,
        payload,
    })
}

fn ctes(owner: &Bound<'_, PyAny>, attribute: &str) -> PyResult<Vec<SqlTestCte>> {
    owner
        .getattr(attribute)?
        .try_iter()?
        .map(|value| {
            let value = value?;
            Ok(SqlTestCte {
                name: native_field(&value, "name")?.extract()?,
                sql_body: native_field(&value, "sql_body")?.extract()?,
            })
        })
        .collect()
}

/// One `SqlValue`, read by its logical kind; decimals keep `as_tuple()`'s digits.
fn parameter_value(value: &Bound<'_, PyAny>) -> PyResult<SqlTestParameterValue> {
    let kind: String = native_field(&native_field(value, "logical_type")?, "kind")?.extract()?;
    let payload = native_field(value, "value")?;
    Ok(match kind.as_str() {
        "string" => SqlTestParameterValue::String(payload.extract()?),
        "integer" => SqlTestParameterValue::Integer(payload.extract()?),
        "boolean" => SqlTestParameterValue::Boolean(payload.extract()?),
        "float" => SqlTestParameterValue::Float(payload.extract()?),
        "decimal" => {
            let (sign, digits, exponent): (u8, Vec<u8>, i64) =
                payload.call_method0("as_tuple")?.extract()?;
            SqlTestParameterValue::Decimal {
                negative: sign == 1,
                digits,
                exponent,
            }
        }
        "null" => SqlTestParameterValue::Null,
        "list" => SqlTestParameterValue::List(parameter_values(&payload)?),
        "set" => SqlTestParameterValue::Set(parameter_values(&payload)?),
        "object" => SqlTestParameterValue::Object(
            payload
                .try_iter()?
                .map(|entry| {
                    let (key, item): (String, Bound<'_, PyAny>) = entry?.extract()?;
                    Ok((key, parameter_value(&item)?))
                })
                .collect::<PyResult<_>>()?,
        ),
        other => {
            return Err(PyErr::new::<PyValueError, _>(format!(
                "unknown SQL value kind {other:?}"
            )));
        }
    })
}

fn parameter_values(items: &Bound<'_, PyAny>) -> PyResult<Vec<SqlTestParameterValue>> {
    items
        .try_iter()?
        .map(|item| parameter_value(&item?))
        .collect()
}

fn assembly_row(outcome: SqlTestAssemblyOutcome) -> AssemblyRow {
    match outcome {
        SqlTestAssemblyOutcome::Assembled(facts) => (Some(facts), None),
        SqlTestAssemblyOutcome::FingerprintOverflow(facts) => {
            (Some(facts), Some((DECIMAL_OVERFLOW, String::new())))
        }
        SqlTestAssemblyOutcome::Failed(SqlTestAssemblyFailure::Input(message)) => {
            (None, Some((INPUT_FAILURE, message)))
        }
        SqlTestAssemblyOutcome::Failed(SqlTestAssemblyFailure::Internal(reason)) => (
            None,
            Some((
                INTERNAL_FAILURE,
                native_failure(SQL_TEST_ASSEMBLY_CONTEXT, &reason),
            )),
        ),
    }
}
