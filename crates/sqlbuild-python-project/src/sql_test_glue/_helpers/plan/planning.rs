//! Plan compiled SQL tests from the compiled project's own objects, without a JSON request.

use crate::sql_test_glue::models::PlanRow;
use sqlbuild_python_base::positional_record;
use std::collections::{BTreeMap, HashMap};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::compiler::main::sql_test_chains::resolve_sql_test_chains;
use sqlbuild_analysis::compiler::main::sql_test_glue::plan_sql_test_batch;
use sqlbuild_analysis::compiler::main::sql_test_plan_errors::sql_test_plan_error_messages;
use sqlbuild_analysis::compiler::models::{
    SqlTestChainBatch, SqlTestChainModel, SqlTestCte, SqlTestPlan, SqlTestPlanBatch,
    SqlTestPlanFunction, SqlTestPlanModel, SqlTestPlanPayload, SqlTestPlanTest,
};

use crate::compiled_views::main::compiled_models::name_object::name_object;
use crate::compiled_views::main::compiled_models::query_sql::query_sql;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_python_front::sqltext::models::LexicalSyntaxInput;

const MODEL_RESOURCE_TYPE: &str = "model";
const PLANNING_WORKERS: usize = 4;

positional_record! {
    extract;
    /// An SQL function's name with its rendered UDF and table-function call affixes.
    pub(crate) struct FunctionRow {
        pub(crate) name: String,
        pub(crate) udf_prefix: String,
        pub(crate) udf_suffix: String,
        pub(crate) table_function_prefix: String,
        pub(crate) table_function_suffix: String,
    }
}

/// The planning request: compiled models and tests, with the adapter's facts.
pub(crate) struct PlanningRequest<'py> {
    pub(crate) models: Vec<Bound<'py, PyAny>>,
    /// Cursor-rendered SQL by model index, for models whose SQL uses cursor intrinsics.
    pub(crate) rendered_model_sql: HashMap<usize, String>,
    pub(crate) functions: Vec<FunctionRow>,
    pub(crate) tests: Vec<Bound<'py, PyAny>>,
    /// Cursor-rendered model query overrides by test index, replacing the test's own.
    pub(crate) rendered_test_overrides: HashMap<usize, BTreeMap<String, String>>,
    pub(crate) sql_analysis_enabled: bool,
    pub(crate) sql_analysis_dialect: Option<String>,
    pub(crate) set_difference_operator: String,
    pub(crate) requires_derived_table_aliases: bool,
    pub(crate) lexical_syntax: LexicalSyntaxInput,
    pub(crate) render_sql: bool,
    pub(crate) include_plan: bool,
    pub(crate) max_unit_test_real_models: Option<usize>,
}

/// The chain request: compiled models and the tests whose chains to resolve.
pub(crate) struct ChainRequest<'py> {
    pub(crate) models: Vec<Bound<'py, PyAny>>,
    pub(crate) tests: Vec<Bound<'py, PyAny>>,
    pub(crate) lexical_syntax: LexicalSyntaxInput,
}

/// Each test's ordered unmocked model chain; direct-logic tests have none.
pub(crate) fn resolve_compiled_sql_test_chains(
    py: Python<'_>,
    request: ChainRequest<'_>,
) -> PyResult<Vec<Vec<String>>> {
    let batch = SqlTestChainBatch {
        models: request
            .models
            .iter()
            .map(|model| {
                Ok(SqlTestChainModel {
                    name: name_object(model)?.extract()?,
                    model_dependencies: model_dependencies(model)?,
                })
            })
            .collect::<PyResult<_>>()?,
        tests: request
            .tests
            .iter()
            .map(|test| plan_test(test, None))
            .collect::<PyResult<_>>()?,
        lexical_syntax: request.lexical_syntax.into(),
    };
    py.compiler_detach(|| resolve_sql_test_chains(batch))
        .map_err(value_error)
}

/// Plan SQL tests in one batch and return each plan with its distinct error messages.
pub(crate) fn plan_compiled_sql_tests(
    py: Python<'_>,
    request: PlanningRequest<'_>,
) -> PyResult<(Vec<PlanRow>, u64, u64)> {
    let batch: SqlTestPlanBatch = planning_batch(request)?;
    let outcome = py
        .compiler_detach(|| plan_sql_test_batch(batch))
        .map_err(value_error)?;
    Ok((
        outcome.plans.into_iter().map(plan_row).collect(),
        outcome.planning_ns,
        outcome.rendering_ns,
    ))
}

fn planning_batch(mut request: PlanningRequest<'_>) -> PyResult<SqlTestPlanBatch> {
    let models: Vec<SqlTestPlanModel> = request
        .models
        .iter()
        .enumerate()
        .map(|(index, model)| plan_model(model, request.rendered_model_sql.remove(&index)))
        .collect::<PyResult<_>>()?;
    let tests: Vec<SqlTestPlanTest> = request
        .tests
        .iter()
        .enumerate()
        .map(|(index, test)| plan_test(test, request.rendered_test_overrides.remove(&index)))
        .collect::<PyResult<_>>()?;
    Ok(SqlTestPlanBatch {
        models,
        functions: request.functions.into_iter().map(plan_function).collect(),
        tests,
        sql_analysis_enabled: request.sql_analysis_enabled,
        sql_analysis_dialect: request.sql_analysis_dialect,
        set_difference_operator: request.set_difference_operator,
        requires_derived_table_aliases: request.requires_derived_table_aliases,
        workers: PLANNING_WORKERS,
        render_sql: request.render_sql,
        include_plan: request.include_plan,
        lexical_syntax: request.lexical_syntax.into(),
        max_unit_test_real_models: request.max_unit_test_real_models,
    })
}

fn plan_model(
    model: &Bound<'_, PyAny>,
    rendered_sql: Option<String>,
) -> PyResult<SqlTestPlanModel> {
    Ok(SqlTestPlanModel {
        name: name_object(model)?.extract()?,
        query_sql: match rendered_sql {
            Some(sql) => sql,
            None => query_sql(model)?.extract()?,
        },
        model_dependencies: model_dependencies(model)?,
    })
}

/// The names of the models among a compiled model's dependencies, in dependency order.
fn model_dependencies(model: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for dependency in native_field(model, "deps")?.try_iter()? {
        let dependency = dependency?;
        if native_field(&dependency, "resource_type")?.eq(MODEL_RESOURCE_TYPE)? {
            names.push(native_field(&dependency, "name")?.extract()?);
        }
    }
    Ok(names)
}

fn plan_function(
    FunctionRow {
        name,
        udf_prefix,
        udf_suffix,
        table_function_prefix,
        table_function_suffix,
    }: FunctionRow,
) -> SqlTestPlanFunction {
    SqlTestPlanFunction {
        name,
        udf_prefix: Some(udf_prefix),
        udf_suffix: Some(udf_suffix),
        table_function_prefix: Some(table_function_prefix),
        table_function_suffix: Some(table_function_suffix),
    }
}

fn plan_test(
    test: &Bound<'_, PyAny>,
    rendered_overrides: Option<BTreeMap<String, String>>,
) -> PyResult<SqlTestPlanTest> {
    let payload = native_field(test, "payload")?;
    let payload: SqlTestPlanPayload = if payload.hasattr("actual_cte")? {
        SqlTestPlanPayload::Direct {
            mode: native_field(&payload, "mode")?.extract()?,
            actual_cte: cte(&native_field(&payload, "actual_cte")?)?,
            expected_cte: cte(&native_field(&payload, "expected_cte")?)?,
            helper_ctes: ctes(&payload, "helper_ctes")?,
        }
    } else {
        SqlTestPlanPayload::Model {
            authored_ctes: ctes(&payload, "authored_ctes")?,
            model_query_overrides: match rendered_overrides {
                Some(overrides) => overrides,
                None => native_field(&payload, "model_query_overrides")?.extract()?,
            },
            expected_ctes: ctes(&payload, "expected_ctes")?,
            expected_model_names: native_field(&payload, "expected_model_names")?.extract()?,
            assertion_ctes: ctes(&payload, "assertion_ctes")?,
            read_helper_names: Some(native_field(test, "read_helper_names")?.extract()?),
            reference_target_model_names: Some(
                native_field(test, "reference_target_model_names")?.extract()?,
            ),
        }
    };
    Ok(SqlTestPlanTest {
        name: native_field(test, "name")?.extract()?,
        file_label: native_field(&native_field(test, "test_file")?, "relative_path")?
            .str()?
            .extract()?,
        payload,
    })
}

fn ctes(owner: &Bound<'_, PyAny>, attribute: &str) -> PyResult<Vec<SqlTestCte>> {
    owner
        .getattr(attribute)?
        .try_iter()?
        .map(|value| cte(&value?))
        .collect()
}

fn cte(value: &Bound<'_, PyAny>) -> PyResult<SqlTestCte> {
    Ok(SqlTestCte {
        name: native_field(value, "name")?.extract()?,
        sql_body: native_field(value, "sql_body")?.extract()?,
    })
}

fn plan_row(plan: SqlTestPlan) -> PlanRow {
    let error_messages: Vec<String> = sql_test_plan_error_messages(&plan);
    PlanRow {
        plan,
        error_messages,
    }
}
