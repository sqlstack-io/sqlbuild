//! The helper and fixture CTEs a scenario's fixtures, expected output and assertions read.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::compiler::main::scenario_helper_scope::scenario_helper_scope as helper_scope;
use sqlbuild_analysis::compiler::models::{ScenarioHelperScope, ScenarioScopeInput};
use sqlbuild_python_base::native_host::types::Syntax;

use crate::planner_scenario::_helpers::values::error;
use crate::planner_scenario::constants::SCENARIO_PLAN_INTERNAL;
use crate::planner_scenario::models::SqlContext;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_front::sqltext::models::LexicalSyntaxInput;

const FIXTURE_LOCATIONS: [(&str, &str); 4] = [
    ("__source__", "source_fixture_locations"),
    ("__ref__", "ref_fixture_locations"),
    ("__seed__", "seed_fixture_locations"),
    ("__dbt_ref__", "dbt_ref_fixture_locations"),
];

/// The scenario's helper scope, where a check reading a fixture by name reads its relation.
pub(crate) fn scenario_helper_scope(
    scenario: &Bound<'_, PyAny>,
    context: &SqlContext<'_>,
) -> PyResult<ScenarioHelperScope> {
    let py = scenario.py();
    let mut ctes: Vec<(String, String)> = Vec::new();
    for cte in native_field(scenario, "authored_ctes")?.try_iter()? {
        let cte = cte?;
        let name: String = native_field(&cte, "name")?.extract()?;
        let body: String = fixture_relation_sql(context, &name)?
            .map_or_else(|| native_field(&cte, "sql_body")?.extract(), Ok)?;
        ctes.push((name, body));
    }
    let syntax: LexicalSyntaxInput = native_field(&context.syntax, "native_mapping")?.extract()?;
    let dialect: Option<String> = context
        .dialect
        .syntax("sql_analysis_dialect_name")?
        .extract()?;
    helper_scope(&ScenarioScopeInput {
        ctes: &ctes,
        checks: &[],
        lexical: syntax.into(),
        dialect: dialect.as_deref(),
        file_label: &context.label,
    })
    .map_err(
        |message| match error(py, &message, SCENARIO_PLAN_INTERNAL, None) {
            Ok(failure) | Err(failure) => failure,
        },
    )
}

/// `SELECT *` from the relation a fixture CTE materialises, or `None` for a helper.
fn fixture_relation_sql(context: &SqlContext<'_>, name: &str) -> PyResult<Option<String>> {
    for (prefix, field) in FIXTURE_LOCATIONS {
        let Some(fixture) = name.strip_prefix(prefix) else {
            continue;
        };
        let location = mapping_get(&native_field(&context.relations, field)?, fixture)?;
        if location.is_none() {
            return Ok(None);
        }
        let relation: String = native_field(&location, "qualified_name")?.extract()?;
        return Ok(Some(format!("SELECT * FROM {relation}")));
    }
    Ok(None)
}
