//! Compiled SQL tests and scenarios read into the native rules request's facts.

use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use sqlbuild_rules::models::{
    DirectTestResourceKind, SqlScenarioFact, SqlTestCteFact, SqlTestFact, SqlTestMode,
    SqlTestParameterFact, SqlTestParameterValueFact, TestedResource,
};

use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_project::main::plain_value::plain_value;
use sqlbuild_python_project::compiled_views::main::project::plain_typed_value::plain_typed_value;

const DESCRIPTION_KEY: &str = "description";

fn posix(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.call_method0("as_posix")?.extract()
}

/// `primary` when it is truthy, otherwise `fallback`, as Python's `or` picks.
fn either<'py>(
    primary: Bound<'py, PyAny>,
    fallback: impl FnOnce() -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    if primary.is_truthy()? {
        Ok(primary)
    } else {
        fallback()
    }
}

fn names(owner: &Bound<'_, PyAny>, attribute: &str) -> PyResult<Vec<String>> {
    field(owner, attribute)?.extract()
}

fn ctes(payload: &Bound<'_, PyAny>, attribute: &str) -> PyResult<Vec<SqlTestCteFact>> {
    field(payload, attribute)?
        .try_iter()?
        .map(|cte| {
            let cte = cte?;
            Ok(SqlTestCteFact {
                name: field(&cte, "name")?.extract()?,
                sql: field(&cte, "sql_body")?.extract()?,
            })
        })
        .collect()
}

fn test_mode(value: &str) -> PyResult<SqlTestMode> {
    match value {
        "model" => Ok(SqlTestMode::Model),
        "macro" => Ok(SqlTestMode::Macro),
        "udf" => Ok(SqlTestMode::Udf),
        "table_fn" => Ok(SqlTestMode::TableFn),
        _ => Err(value_error(format!("unknown SQL test mode {value}"))),
    }
}

fn tested_kind(value: &str) -> PyResult<DirectTestResourceKind> {
    match value {
        "macro" => Ok(DirectTestResourceKind::Macro),
        "udf" => Ok(DirectTestResourceKind::Udf),
        "table_fn" => Ok(DirectTestResourceKind::TableFn),
        _ => Err(value_error(format!("unknown tested resource kind {value}"))),
    }
}

fn tested_resources(test: &Bound<'_, PyAny>) -> PyResult<Vec<TestedResource>> {
    field(test, "tested_resources")?
        .try_iter()?
        .map(|resource| {
            let resource = resource?;
            let kind: String = field(&resource, "kind")?.extract()?;
            Ok(TestedResource {
                kind: tested_kind(&kind)?,
                name: field(&resource, "name")?.extract()?,
            })
        })
        .collect()
}

fn with_payload(test: &Bound<'_, PyAny>, mut fact: SqlTestFact) -> PyResult<SqlTestFact> {
    let payload: Bound<'_, PyAny> = field(test, "payload")?;
    let model_payload: Bound<'_, PyAny> =
        record_class(test.py(), RecordId::CompiledModelSqlTestPayload)?;
    if !payload.is_instance(&model_payload)? {
        return Ok(fact);
    }
    fact.authored_ctes = ctes(&payload, "authored_ctes")?;
    fact.expected_ctes = ctes(&payload, "expected_ctes")?;
    fact.assertion_ctes = ctes(&payload, "assertion_ctes")?;
    fact.has_macro_mocks = field(&payload, "macro_mocks")?.is_truthy()?;
    fact.has_model_query_overrides = field(&payload, "model_query_overrides")?.is_truthy()?;
    Ok(fact)
}

fn with_case(test: &Bound<'_, PyAny>, mut fact: SqlTestFact) -> PyResult<SqlTestFact> {
    let case_name: Option<String> = field(test, "case_name")?.extract()?;
    let Some(case_name) = case_name else {
        return Ok(fact);
    };
    let mut schema: Vec<SqlTestParameterFact> = Vec::new();
    for parameter in field(test, "parameter_schema")?.try_iter()? {
        let parameter = parameter?;
        schema.push(SqlTestParameterFact {
            name: field(&parameter, "name")?.extract()?,
            value_type: field(&parameter, "value_type")?.extract()?,
            nullable: field(&parameter, "nullable")?.extract()?,
        });
    }
    let types: HashMap<&str, &str> = schema
        .iter()
        .map(|parameter| (parameter.name.as_str(), parameter.value_type.as_str()))
        .collect();
    let mut parameters: Vec<SqlTestParameterValueFact> = Vec::new();
    for item in field(test, "parameter_values")?.try_iter()? {
        let (name, value): (String, Bound<'_, PyAny>) = item?.extract()?;
        let value_type: String = types
            .get(name.as_str())
            .ok_or_else(|| PyValueError::new_err(format!("unknown test parameter {name}")))?
            .to_string();
        parameters.push(SqlTestParameterValueFact {
            name,
            value_type,
            value: plain_value(&plain_typed_value(&value)?)?,
        });
    }
    fact.parameters = parameters;
    fact.parameter_schema = schema;
    fact.parent_name = field(test, "parent_name")?.extract()?;
    fact.case_name = Some(case_name);
    fact.case_index = field(test, "case_index")?.extract()?;
    fact.case_fingerprint = field(test, "case_fingerprint")?.extract()?;
    Ok(fact)
}

/// One compiled SQL test as the facts the built-in rules read.
pub(crate) fn sql_test_fact(test: &Bound<'_, PyAny>) -> PyResult<SqlTestFact> {
    let test_file: Bound<'_, PyAny> = field(test, "test_file")?;
    let source_path: Bound<'_, PyAny> = either(field(test, "source_path")?, || {
        field(&test_file, "relative_path")
    })?;
    let ownership_root: Bound<'_, PyAny> = either(field(test, "ownership_root")?, || {
        field(&test_file, "ownership_root")
    })?;
    let block_index: Bound<'_, PyAny> = either(field(test, "block_index")?, || {
        field(&field(test, "test_block")?, "test_index")
    })?;
    let mode: String = field(test, "mode")?.extract()?;
    let fact = SqlTestFact {
        source_path: posix(&source_path)?,
        ownership_root: posix(&ownership_root)?,
        block_index: block_index.extract()?,
        name: field(test, "name")?.extract()?,
        explicit_name: field(test, "explicit_name")?.extract()?,
        parent_name: None,
        case_name: None,
        case_index: None,
        case_fingerprint: None,
        parameter_schema: Vec::new(),
        parameters: Vec::new(),
        mode: test_mode(&mode)?,
        expected_model_names: names(test, "expected_model_names")?,
        assertion_names: names(test, "assertion_names")?,
        assertion_target_model_names: names(test, "assertion_target_model_names")?,
        target_model_names: names(test, "target_model_names")?,
        tested_resources: tested_resources(test)?,
        authored_ctes: Vec::new(),
        expected_ctes: Vec::new(),
        assertion_ctes: Vec::new(),
        has_macro_mocks: false,
        has_model_query_overrides: false,
        empty_input_only: false,
    };
    with_case(test, with_payload(test, fact)?)
}

/// One compiled SQL scenario as the facts the built-in rules read.
pub(crate) fn scenario_fact(scenario: &Bound<'_, PyAny>) -> PyResult<SqlScenarioFact> {
    let scenario_file: Bound<'_, PyAny> = field(scenario, "scenario_file")?;
    let source_path: Bound<'_, PyAny> = either(field(scenario, "source_path")?, || {
        field(&scenario_file, "relative_path")
    })?;
    let ownership_root: Bound<'_, PyAny> = either(field(scenario, "ownership_root")?, || {
        field(&scenario_file, "ownership_root")
    })?;
    let description: Option<String> = match field(&scenario_file, "header_values")?
        .cast_into::<PyDict>()?
        .get_item(DESCRIPTION_KEY)?
    {
        Some(header) if header.is_instance_of::<PyString>() => Some(header.extract()?),
        _ => None,
    };
    Ok(SqlScenarioFact {
        source_path: posix(&source_path)?,
        ownership_root: posix(&ownership_root)?,
        name: field(scenario, "name")?.extract()?,
        description,
        expected_model_names: names(scenario, "expected_model_names")?,
        assertion_names: names(scenario, "assertion_names")?,
        assertion_target_model_names: names(scenario, "assertion_target_model_names")?,
        target_model_names: names(scenario, "target_model_names")?,
    })
}
