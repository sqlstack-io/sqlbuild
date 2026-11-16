//! Assemble existing SQL-test plan dataclasses without changing executor ownership.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyListMethods, PyTuple};
use sqlbuild_model_config::execution_limits::constants::TIMEOUT_KEY as TIMEOUT_HEADER_KEY;
use sqlbuild_model_config::execution_limits::main::parse_execution_limit::parse_execution_limit;
use sqlbuild_model_config::execution_limits::models::ExecutionLimit;

use crate::planner_strategy::constants::MODEL_REFERENCE_KIND;
use crate::planner_strategy::models::RelationFixturePlanningContext;
use crate::sql_test_glue::_helpers::plan::plan_values::model_payload;
use crate::sql_test_glue::constants::{TABLE_FN, UDF};
use sqlbuild_analysis::compiler::main::missing_expected_columns::missing_expected_columns;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::record_class::record_class;

use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::sql_test_glue::main::prefixed_ctes::prefixed_ctes;

fn prefixed_names<'py>(test: &Bound<'py, PyAny>, prefix: &str) -> PyResult<Bound<'py, PyAny>> {
    let py = test.py();
    let mut names: Vec<String> = if model_payload(test)? {
        prefixed_ctes(
            &native_field(&native_field(test, "payload")?, "authored_ctes")?,
            prefix,
        )?
        .keys()
        .extract()?
    } else {
        Vec::new()
    };
    names.sort();
    Ok(PyTuple::new(py, names)?.into_any())
}

fn function_deps<'py>(
    test: &Bound<'py, PyAny>,
    plan: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
    inputs: (&Bound<'py, PyAny>, &Bound<'py, PyDict>),
) -> PyResult<Bound<'py, PyAny>> {
    let py = test.py();
    let deps = PyDict::new(py);
    if model_payload(test)? {
        let names: Vec<String> = native_field(plan, "model_names")?.extract()?;
        let mocked: std::collections::HashSet<String> =
            inputs.0.extract::<Vec<String>>()?.into_iter().collect();
        let models = inputs.1;
        for name in names {
            let Some(model) = models.get_item(name)? else {
                continue;
            };
            for dep in native_field(&model, "deps")?.try_iter()? {
                let dep = dep?;
                let resource: String =
                    native_field(&native_field(&dep, "resource_type")?, "value")?.extract()?;
                if resource != UDF && resource != TABLE_FN {
                    continue;
                }
                if resource == TABLE_FN
                    && mocked.contains(&native_field(&dep, "name")?.extract::<String>()?)
                {
                    continue;
                }
                deps.set_item(dep, py.None())?;
            }
        }
    } else {
        let payload = native_field(test, "payload")?;
        let mode: String = native_field(&native_field(&payload, "mode")?, "value")?.extract()?;
        if mode == UDF || mode == TABLE_FN {
            let names: std::collections::HashSet<String> =
                native_field(&payload, "tested_resource_names")?
                    .extract::<Vec<String>>()?
                    .into_iter()
                    .collect();
            for function in native_field(project, "functions")?.try_iter()? {
                let function = function?;
                let key = native_field(&function, "key")?;
                if native_field(&native_field(&key, "resource_type")?, "value")?
                    .extract::<String>()?
                    == mode
                    && names.contains(&native_field(&function, "name")?.extract::<String>()?)
                {
                    deps.set_item(key, py.None())?;
                }
            }
        }
    }
    Ok(PyTuple::new(py, deps.keys().iter())?.into_any())
}

pub(crate) fn entry<'py>(
    test: &Bound<'py, PyAny>,
    plan: &Bound<'py, PyAny>,
    inputs: (&Bound<'py, PyAny>, &Bound<'py, PyDict>),
    analysis: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = test.py();
    let fields = PyDict::new(py);
    for name in [
        "key",
        "name",
        "source_path",
        "block_index",
        "parent_name",
        "case_name",
        "case_index",
        "case_fingerprint",
        "parameter_schema",
        "parameter_values",
        "scope_deps",
    ] {
        fields.set_item(name, native_field(test, name)?)?;
    }
    for (field, prefix) in [
        ("mock_ref_names", "__ref__"),
        ("mock_source_names", "__source__"),
        ("mock_seed_names", "__seed__"),
        ("mock_dbt_ref_names", "__dbt_ref__"),
        ("mock_table_function_names", "__table_fn__"),
    ] {
        fields.set_item(field, prefixed_names(test, prefix)?)?;
    }
    let mocks = fields
        .get_item("mock_table_function_names")?
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("missing mock names"))?;
    fields.set_item(
        "function_deps",
        function_deps(test, plan, inputs.0, (&mocks, inputs.1))?,
    )?;
    let chain = native_field(plan, "chain")?;
    let direct: bool = native_field(test, "payload")?.hasattr("actual_cte")?;
    fields.set_item(
        "real_model_count",
        if direct { None } else { Some(chain.len()?) },
    )?;
    let header: Bound<'py, PyDict> =
        native_field(&native_field(test, "test_block")?, "header_values")?.cast_into::<PyDict>()?;
    let timeout: Option<String> = match header.get_item(TIMEOUT_HEADER_KEY)? {
        Some(value) => value.extract()?,
        None => None,
    };
    let timeout_seconds: Option<u64> = match timeout.as_deref().and_then(parse_execution_limit) {
        Some(ExecutionLimit::Seconds(seconds)) => Some(seconds),
        Some(ExecutionLimit::Disabled) | None => None,
    };
    fields.set_item("timeout", timeout)?;
    fields.set_item("timeout_seconds", timeout_seconds)?;
    fields.set_item("chain", chain)?;
    fields.set_item("assertions", native_field(plan, "assertions")?)?;
    fields.set_item("sql_analysis_enabled", analysis)?;
    new_record(py, RecordId::SqlTestPlanEntry, (), Some(&fields))
}

pub(crate) fn expected_diagnostics(
    test: &Bound<'_, PyAny>,
    plan: &Bound<'_, PyAny>,
    context: Option<&RelationFixturePlanningContext>,
) -> PyResult<Vec<String>> {
    let mut result: Vec<String> = Vec::new();
    let Some(context) = context else {
        return Ok(result);
    };
    let py = test.py();
    let resource_types = record_class(py, RecordId::CompiledResourceType)?;
    for step in native_field(plan, "chain")?.try_iter()? {
        let step = step?;
        let name = native_field(&step, "model_name")?;
        let model_name: String = name.extract()?;
        let available = context
            .authoritative_columns
            .get(&(MODEL_REFERENCE_KIND.to_owned(), model_name));
        let expected = native_field(&step, "expected_columns")?;
        let Some(available) = available else {
            continue;
        };
        if expected.is_none() {
            continue;
        }
        let available: Vec<String> = available.iter().cloned().collect();
        let expected: Vec<String> = expected.extract()?;
        let Some(message) =
            missing_expected_columns(&name.extract::<String>()?, &expected, &available)
        else {
            continue;
        };
        let location =
            crate::sql_test_glue::_helpers::inputs::fixture_validation::fixture_location(
                test,
                &native_field(&resource_types, "SQL_TEST")?,
                &name.extract::<String>()?,
            )?;
        result.push(format!(
            "SQL test '{}': {}: {}",
            native_field(test, "name")?.extract::<String>()?,
            location,
            message
        ));
    }
    Ok(result)
}
