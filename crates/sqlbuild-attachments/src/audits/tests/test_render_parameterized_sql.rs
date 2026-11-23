use crate::audits::_helpers::parameters::RenderStop;
use crate::audits::main::render_parameterized_sql::render_parameterized_sql;
use crate::audits::models::{ArgumentValue, ParameterRenderStop};
use crate::audits::tests::helpers::rendered_sql;
use crate::audits::tests::test_types::{ParameterizedSqlTestCase, UnusedArgumentsTestCase};

#[test]
fn given_parameterized_sql_when_rendering_then_python_text_is_returned() {
    let test_cases = [
        ParameterizedSqlTestCase {
            description: "quoted inserted argument markers are not rescanned",
            sql: "SELECT @'label'",
            expected_sql: Ok("SELECT 'O''Brien@orders'"),
        },
        ParameterizedSqlTestCase {
            description: "raw and quoted parameters: lists joined, only text quoted, quotes doubled",
            sql: "WHERE @column IN (@'values') AND x = @values",
            expected_sql: Ok(
                "WHERE status IN ('it''s', 2, NULL, TRUE) AND x = it's, 2, NULL, TRUE",
            ),
        },
        ParameterizedSqlTestCase {
            description: "macro calls, doubled @ and runtime placeholders are left alone",
            sql: "@py.column (x) @@column @@@column @py.column(y)",
            expected_sql: Ok("@py.column (x) @@column @@@column @py.column(y)"),
        },
        ParameterizedSqlTestCase {
            description: "the first missing argument stops rendering with its name",
            sql: "WHERE @column = @missing AND @'other'",
            expected_sql: Err(RenderStop::MissingArgument("missing".to_owned())),
        },
        ParameterizedSqlTestCase {
            description: "Unicode whitespace before a parenthesis makes a macro call",
            sql: "WHERE @column\u{a0}( AND @column\u{e9}",
            expected_sql: Ok("WHERE @column\u{a0}( AND status\u{e9}"),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            rendered_sql(test_case.sql),
            test_case.expected_sql.map(str::to_owned),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_unreferenced_arguments_when_rejecting_unused_then_their_sorted_names_stop_rendering() {
    let test_cases = [
        UnusedArgumentsTestCase {
            description: "unused names are reported sorted before a missing argument",
            sql: "GRANT SELECT TO @'role' AND @missing",
            arguments: &["scope", "role", "mode"],
            expected_sql: Err(ParameterRenderStop::UnusedArguments(vec![
                "mode".to_owned(),
                "scope".to_owned(),
            ])),
        },
        UnusedArgumentsTestCase {
            description: "every argument referenced renders",
            sql: "GRANT SELECT TO @'role'",
            arguments: &["role"],
            expected_sql: Ok("GRANT SELECT TO 'role-value'"),
        },
    ];

    for test_case in test_cases {
        let arguments: Vec<(String, ArgumentValue)> = test_case
            .arguments
            .iter()
            .map(|name| {
                (
                    (*name).to_owned(),
                    ArgumentValue::Text(format!("{name}-value")),
                )
            })
            .collect();
        assert_eq!(
            render_parameterized_sql(test_case.sql, &arguments, (true, None)),
            test_case.expected_sql.map(str::to_owned),
            "{}",
            test_case.description
        );
    }
}
