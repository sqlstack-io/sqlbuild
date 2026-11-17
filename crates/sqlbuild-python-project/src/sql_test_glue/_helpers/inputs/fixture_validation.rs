//! Static SQL-test fixture shape validation over native fixture completion.

use sqlbuild_python_base::native_host::models::Dialect;
use std::collections::BTreeSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_analysis::planner_fixtures::main::{
    collection_mismatch::collection_mismatch, fixture_line::fixture_line,
};

use crate::planner_strategy::constants::MODEL_REFERENCE_KIND;
use crate::planner_strategy::main::complete_empty_fixture_sql::complete_empty_fixture_sql;
use crate::planner_strategy::main::complete_fixtures::complete_fixtures;
use crate::planner_strategy::main::complete_typed_null_columns::complete_typed_null_columns;
use crate::planner_strategy::models::RelationFixturePlanningContext;
use crate::planner_strategy::types::FixtureKey;
use crate::sql_test_glue::constants::FIXTURE_PREFIXES;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_base::records::types::RecordId;

/// Report every statically provable fixture defect or return the completed fixture SQL groups.
pub(crate) fn validated_fixtures<'py>(
    test: &Bound<'py, PyAny>,
    request: (&Bound<'py, PyAny>, [Bound<'py, PyDict>; 4]),
    inputs: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    context: &RelationFixturePlanningContext,
) -> PyResult<Result<[Bound<'py, PyDict>; 4], Vec<String>>> {
    let py = test.py();
    let (chain, [refs, sources, seeds, expected]) = request;
    let dialect = Dialect::of(inputs.1)?;
    let dialect = &dialect;
    let resource_types = record_class(py, RecordId::CompiledResourceType)?;
    let model = native_field(&resource_types, "MODEL")?;
    let kinds = [
        model.clone(),
        native_field(&resource_types, "SOURCE")?,
        native_field(&resource_types, "SEED")?,
    ];
    let groups = PyTuple::new(
        py,
        [
            PyTuple::new(py, [kinds[0].clone(), refs.clone().into_any()])?,
            PyTuple::new(py, [kinds[1].clone(), sources.clone().into_any()])?,
            PyTuple::new(py, [kinds[2].clone(), seeds.clone().into_any()])?,
        ],
    )?;
    let chain: Vec<String> = chain.extract()?;
    let completion = complete_fixtures(context, dialect, (&chain, groups.as_any()))?;
    let mut errors: BTreeSet<String> = BTreeSet::new();
    for diagnostic in &completion.diagnostics {
        let key = native_field(diagnostic, "key")?;
        errors.insert(format!(
            "{}: {}",
            fixture_location(
                test,
                &key.get_item(0)?,
                &key.get_item(1)?.extract::<String>()?
            )?,
            native_field(diagnostic, "message")?.extract::<String>()?
        ));
    }
    let inferred_by_fixture = &completion.inferred_by_fixture;
    let completed_expected = expected.copy()?;
    let sql_test = native_field(&resource_types, "SQL_TEST")?;
    let type_groups = [
        (&kinds[0], &refs),
        (&kinds[1], &sources),
        (&kinds[2], &seeds),
        (&sql_test, &expected),
    ];
    for (kind, fixtures) in type_groups {
        let expected_output = kind.is(&sql_test);
        for (key_name, sql) in fixtures.iter() {
            let name: String = key_name.extract()?;
            let location = || fixture_location(test, kind, &name);
            let inferred = if expected_output {
                let model_key: FixtureKey = (MODEL_REFERENCE_KIND.to_owned(), name.clone());
                let (completed_sql, empty_error) = complete_empty_fixture_sql(
                    &sql.extract::<String>()?,
                    context.relations.get(&model_key),
                    dialect,
                )?;
                if let Some(error) = empty_error {
                    errors.insert(format!("{}: expected output '{name}' {error}", location()?));
                }
                completed_expected.set_item(&name, &completed_sql)?;
                let kwargs = PyDict::new(py);
                kwargs.set_item("query_sql", &completed_sql)?;
                kwargs.set_item(
                    "inference_profile",
                    dialect.call_method0("expression_inference_profile")?,
                )?;
                let inference =
                    context
                        .fixture_inferences
                        .bind(py)
                        .call_method("infer", (), Some(&kwargs))?;
                if inference.is_none() {
                    None
                } else {
                    let types: &[(String, String)] = completion
                        .expected_types
                        .get(&model_key)
                        .unwrap_or_default();
                    completed_expected.set_item(
                        &name,
                        complete_typed_null_columns(&completed_sql, &inference, types, dialect)?,
                    )?;
                    Some(native_field(&inference, "columns")?)
                }
            } else {
                let key = PyTuple::new(py, [kind, &key_name])?;
                Some(mapping_get(inferred_by_fixture, key)?).filter(|value| !value.is_none())
            };
            let Some(columns) = inferred else {
                continue;
            };
            let type_kind: String = if expected_output {
                MODEL_REFERENCE_KIND.to_owned()
            } else {
                kind.extract()?
            };
            let types: &[(String, String)] = completion
                .expected_types
                .get(&(type_kind, name.clone()))
                .unwrap_or_default();
            for column in columns.try_iter()? {
                let column = column?;
                let column_name = native_field(&column, "name")?;
                let folded_name: String = column_name.call_method0("casefold")?.extract()?;
                let expected_type: Option<String> = types
                    .iter()
                    .find(|(column, _)| *column == folded_name)
                    .map(|(_, sql_type)| sql_type.clone());
                let inferred_type: Option<String> = native_field(&column, "type")?.extract()?;
                if !collection_mismatch(expected_type.as_deref(), inferred_type.as_deref()) {
                    continue;
                }
                let label = if expected_output {
                    "expected output".to_owned()
                } else {
                    format!("mock {}", native_field(kind, "value")?.extract::<String>()?)
                };
                errors.insert(format!(
                    "{}: {label} '{name}' column '{}' has incompatible type {}; resource type is {}. Use an explicit CAST, ARRAY_CONSTRUCT, or PARSE_JSON as appropriate",
                    location()?,
                    column_name.extract::<String>()?,
                    inferred_type.unwrap_or_default(),
                    expected_type.unwrap_or_default(),
                ));
            }
        }
    }
    if !errors.is_empty() {
        let test_name: String = native_field(test, "name")?.extract()?;
        return Ok(Err(errors
            .into_iter()
            .map(|error| format!("SQL test '{test_name}': {error}"))
            .collect()));
    }
    let fixture_sql = &completion.sql_by_key;
    let completed =
        |kind: &Bound<'py, PyAny>, fixtures: &Bound<'py, PyDict>| -> PyResult<Bound<'py, PyDict>> {
            let result = PyDict::new(py);
            for (name, sql) in fixtures.iter() {
                let key = PyTuple::new(py, [kind.clone(), name.clone()])?;
                result.set_item(&name, mapping_get_or(fixture_sql, key, sql)?)?;
            }
            Ok(result)
        };
    Ok(Ok([
        completed(&kinds[0], &refs)?,
        completed(&kinds[1], &sources)?,
        completed(&kinds[2], &seeds)?,
        completed_expected,
    ]))
}

/// Return the test file and line that defines one fixture or expected-output CTE.
pub(crate) fn fixture_location(
    test: &Bound<'_, PyAny>,
    resource_type: &Bound<'_, PyAny>,
    name: &str,
) -> PyResult<String> {
    let value: String = native_field(resource_type, "value")?.extract()?;
    let prefix = FIXTURE_PREFIXES
        .iter()
        .find_map(|(kind, prefix)| (*kind == value).then_some(*prefix))
        .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(value))?;
    let test_file = native_field(test, "test_file")?;
    let line = fixture_line(
        &native_field(&test_file, "contents")?.extract::<String>()?,
        &format!("{prefix}{name}"),
    )
    .map_err(value_error)?;
    Ok(format!(
        "{}:{line}",
        native_field(&test_file, "relative_path")?
            .call_method0("as_posix")?
            .extract::<String>()?
    ))
}
