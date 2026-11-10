//! Compare a scenario `__expected__` CTE's listed columns with its model's output columns.

use crate::compiler::_helpers::sql_tests::expected_columns::{
    expected_columns, normalized_identifier,
};
use crate::compiler::_helpers::sql_tests::rendering::sql_test_parser;
use crate::compiler::models::ScenarioExpectedColumns;

/// The columns an expected CTE lists and how they differ from the known model output.
#[must_use]
pub fn scenario_expected_columns(
    expected_sql: &str,
    dialect: Option<&str>,
    model_columns: Option<&[String]>,
) -> ScenarioExpectedColumns {
    let listed: Option<Vec<String>> =
        sql_test_parser(dialect).and_then(|parser| expected_columns(expected_sql, parser));
    let (Some(listed_columns), Some(model_columns)) = (listed.as_deref(), model_columns) else {
        return ScenarioExpectedColumns {
            listed,
            unlisted: Vec::new(),
            unknown: Vec::new(),
        };
    };
    let listed_names: Vec<String> = listed_columns
        .iter()
        .map(|column| normalized_identifier(column))
        .collect();
    let model_names: Vec<String> = model_columns
        .iter()
        .map(|column| normalized_identifier(column))
        .collect();
    let unlisted: Vec<String> = model_columns
        .iter()
        .zip(&model_names)
        .filter(|(_, name)| !listed_names.contains(name))
        .map(|(column, _)| column.clone())
        .collect();
    let unknown: Vec<String> = listed_columns
        .iter()
        .zip(&listed_names)
        .filter(|(_, name)| !model_names.contains(name))
        .map(|(column, _)| column.clone())
        .collect();
    ScenarioExpectedColumns {
        listed,
        unlisted,
        unknown,
    }
}
