//! Scenario helper and fixture CTEs indexed through the SQL-test helper scope graph.

use std::collections::BTreeMap;

use crate::compiler::_helpers::sql_tests::cte_slices::SliceDialect;
use crate::compiler::_helpers::sql_tests::helper_scope::{ScopeGraph, ScopeRequest};
use crate::compiler::_helpers::sql_tests::planning::{
    ASSERT_PREFIX, DBT_REF_PREFIX, EXPECTED_PREFIX, REF_PREFIX, SEED_PREFIX, SOURCE_PREFIX,
    SqlTestPatterns, TestFixtures,
};
use crate::compiler::models::{ScenarioHelperScope, ScenarioScopeInput, SqlTestCte};

const ERROR_PREFIX: &str = "compile_input:";
const FIXTURE_PREFIXES: [&str; 4] = [REF_PREFIX, SOURCE_PREFIX, SEED_PREFIX, DBT_REF_PREFIX];

/// Index every helper, including ones that call references, and every fixture as a mock.
pub(crate) fn helper_scope(input: &ScenarioScopeInput<'_>) -> Result<ScenarioHelperScope, String> {
    let patterns = SqlTestPatterns::new(input.lexical.clone()).map_err(plain)?;
    let fixtures = scenario_fixtures(input.ctes, input.checks);
    let read_helper_names: Vec<String> = fixtures
        .helpers
        .iter()
        .map(|cte| cte.name.clone())
        .collect();
    let graph = ScopeGraph::new(&ScopeRequest {
        fixtures: &fixtures,
        patterns: &patterns,
        file_label: input.file_label,
        rename_dialect: None,
        flat_dialect: true,
        slice_dialect: SliceDialect::new(input.dialect),
        read_helper_names: &read_helper_names,
    })
    .map_err(plain)?;
    Ok(ScenarioHelperScope { graph, patterns })
}

fn scenario_fixtures(ctes: &[(String, String)], checks: &[(String, String)]) -> TestFixtures {
    let mut fixtures = TestFixtures {
        mock_refs: BTreeMap::new(),
        mock_sources: BTreeMap::new(),
        mock_seeds: BTreeMap::new(),
        mock_dbt_refs: BTreeMap::new(),
        mock_table_functions: BTreeMap::new(),
        mock_table_function_columns: BTreeMap::new(),
        helpers: Vec::new(),
        scope: ScopeGraph::default(),
        expected: BTreeMap::new(),
        assertions: Vec::new(),
    };
    for (name, sql) in ctes {
        let fixture: Option<(&str, &str)> = fixture_key(name);
        let group = match fixture {
            Some((REF_PREFIX, _)) => &mut fixtures.mock_refs,
            Some((SOURCE_PREFIX, _)) => &mut fixtures.mock_sources,
            Some((SEED_PREFIX, _)) => &mut fixtures.mock_seeds,
            Some(_) => &mut fixtures.mock_dbt_refs,
            None => {
                fixtures.helpers.push(SqlTestCte {
                    name: name.clone(),
                    sql_body: sql.clone(),
                });
                continue;
            }
        };
        if let Some((_, key)) = fixture {
            group.insert(key.to_owned(), sql.clone());
        }
    }
    for (name, sql) in checks {
        if let Some(model) = name.strip_prefix(EXPECTED_PREFIX) {
            fixtures.expected.insert(model.to_owned(), sql.clone());
        } else if let Some(assertion) = name.strip_prefix(ASSERT_PREFIX) {
            fixtures
                .assertions
                .push((assertion.to_owned(), sql.clone()));
        }
    }
    fixtures
}

/// The fixture prefix and name of a fixture CTE, or `None` for a helper.
fn fixture_key(name: &str) -> Option<(&'static str, &str)> {
    for prefix in FIXTURE_PREFIXES {
        if let Some(key) = name.strip_prefix(prefix) {
            return Some((prefix, key));
        }
    }
    None
}

fn plain(message: String) -> String {
    message
        .strip_prefix(ERROR_PREFIX)
        .map_or_else(|| message.clone(), str::to_owned)
}
