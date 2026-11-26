//! SQL test macro-mock model queries and compiled SQL scenarios of an assembled project.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyString, PyTuple};

use pyo3::IntoPyObjectExt;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::attachments::main::validated_model_cursor_sql::validated_model_cursor_sql;
use sqlbuild_python_front::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use sqlbuild_python_front::render::main::declarations::declaration_scope_resolver::declaration_scope_resolver;
use sqlbuild_python_front::render::main::expansion::expansion_result::expansion_result;
use sqlbuild_python_front::render::main::sql_macros::with_sql_macro_mocks::with_sql_macro_mocks;
use sqlbuild_python_front::render::models::ExpansionRequest;

/// The `CompiledSqlScenario` fields copied from its input, in Python's keyword order.
const SCENARIO_FIELDS: [&str; 12] = [
    "sql_body",
    "authored_ctes",
    "expected_ctes",
    "assertion_ctes",
    "source_fixture_names",
    "ref_fixture_names",
    "seed_fixture_names",
    "dbt_ref_fixture_names",
    "expected_model_names",
    "assertion_names",
    "assertion_target_model_names",
    "target_model_names",
];

/// Re-expand each model from its pre-macro SQL with the test's macro mocks applied.
pub(crate) fn test_model_query_overrides<'py>(
    test_input: &Bound<'py, PyAny>,
    inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = inputs.py();
    let overrides: Bound<'py, PyDict> = PyDict::new(py);
    let payload: Bound<'py, PyAny> = native_field(test_input, "payload")?;
    if !payload.is_instance(&record_class(
        py,
        RecordId::CompileModelSqlTestInputPayload,
    )?)? {
        return Ok(overrides);
    }
    let macro_mocks: Bound<'py, PyAny> = native_field(&payload, "macro_mocks")?;
    if !macro_mocks.is_truthy()? {
        return Ok(overrides);
    }
    let macro_context: Bound<'py, PyAny> = replaced(
        &model_macro_context(inputs)?,
        &[(
            "_enforce_explicit_references",
            PyBool::new(py, false).to_owned().into_any(),
        )],
    )?;
    let declaration_scope: Bound<'py, PyAny> = native_field(inputs, "declaration_scope")?;
    let lookup: Bound<'py, PyAny> = if declaration_scope.is_none() {
        py.None().into_bound(py)
    } else {
        native_field(&declaration_scope, "resolver")?.getattr("lookup")?
    };
    let loaded_macros: Bound<'py, PyAny> = native_field(inputs, "loaded_macros")?;
    let declaration_resolver: Bound<'py, PyAny> = declaration_scope_resolver(
        &native_field(inputs, "discovered_inputs")?,
        &native_field(inputs, "scope_index")?,
        (&loaded_macros, &lookup),
    )?;
    for model_input in native_field(inputs, "model_inputs")?.try_iter()? {
        let model_input: Bound<'py, PyAny> = model_input?;
        let file_path: Bound<'py, PyAny> =
            native_field(&model_input, "model_file")?.getattr("file_path")?;
        let model_name: Bound<'py, PyAny> = native_field(&file_path, "stem")?;
        let macro_source_sql: Bound<'py, PyAny> = native_field(&model_input, "macro_source_sql")?;
        let macro_source_sql: Bound<'py, PyAny> = if macro_source_sql.is_truthy()? {
            macro_source_sql
        } else {
            native_field(&model_input, "query_sql")?
        };
        let macro_source_sql: String = with_sql_macro_mocks(
            &model_input,
            macro_source_sql.extract()?,
            (macro_mocks.cast()?, &loaded_macros),
        )?;
        let expanded: Bound<'py, PyAny> = expansion_result(ExpansionRequest {
            sql: PyString::new(py, &macro_source_sql),
            file_path,
            loaded_macros: loaded_macros.cast()?.clone(),
            macro_overrides: Some(macro_mocks.cast()?.clone()),
            macro_context: macro_context.clone(),
            declaration_resolver: (!declaration_resolver.is_none())
                .then(|| declaration_resolver.clone()),
            declarations: None,
            consumer: None,
        })?
        .getattr("sql")?;
        let query: String = validated_model_cursor_sql(
            py,
            &expanded.extract::<String>()?,
            (
                &model_name.extract::<String>()?,
                &native_field(&model_input, "config")?.getattr("values")?,
            ),
        )?;
        overrides.set_item(model_name, query)?;
    }
    Ok(overrides)
}

/// The compile's macro context, or one built from the effective adapter, settings and target.
fn model_macro_context<'py>(inputs: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = inputs.py();
    let macro_context: Bound<'py, PyAny> = native_field(inputs, "macro_context")?;
    if macro_context.is_truthy()? {
        return Ok(macro_context);
    }
    let adapter_name: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &native_field(inputs, "project_config")?,
        &native_field(inputs, "local_config")?,
    )
    .and_then(|value| value.into_bound_py_any(py))?;
    new_record_with(
        py,
        RecordId::MacroContext,
        &[
            ("adapter_name", adapter_name),
            (
                "sql_analysis_enabled",
                native_field(inputs, "effective_settings")?.getattr("sql_analysis")?,
            ),
            (
                "target_name",
                native_field(inputs, "effective_target_name")?,
            ),
            ("vars", native_field(inputs, "effective_vars")?),
        ],
    )
}

/// Each scenario input as a `CompiledSqlScenario`.
pub(crate) fn compiled_sql_scenarios<'py>(
    inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = inputs.py();
    let resource_type: Bound<'py, PyAny> =
        enum_member(py, RecordId::CompiledResourceType, "SQL_SCENARIO")?;
    let mut scenarios: Vec<Bound<'py, PyAny>> = Vec::new();
    for scenario_input in native_field(inputs, "scenario_inputs")?.try_iter()? {
        let scenario_input: Bound<'py, PyAny> = scenario_input?;
        let scenario_file: Bound<'py, PyAny> = native_field(&scenario_input, "scenario_file")?;
        let name: Bound<'py, PyAny> = native_field(&scenario_file, "name")?;
        let key: Bound<'py, PyAny> = new_record_with(
            py,
            RecordId::CompiledObjectKey,
            &[
                ("resource_type", resource_type.clone()),
                ("name", name.clone()),
            ],
        )?;
        let fields: Bound<'py, PyDict> = keywords(
            py,
            &[
                ("key", key),
                ("name", name),
                ("scenario_file", scenario_file.clone()),
            ],
        )?;
        for field in SCENARIO_FIELDS {
            fields.set_item(field, native_field(&scenario_input, field)?)?;
        }
        fields.set_item(
            "source_path",
            native_field(&scenario_file, "relative_path")?,
        )?;
        fields.set_item(
            "ownership_root",
            native_field(&scenario_file, "ownership_root")?,
        )?;
        scenarios.push(new_record(
            py,
            RecordId::CompiledSqlScenario,
            (),
            Some(&fields),
        )?);
    }
    PyTuple::new(py, scenarios)
}
