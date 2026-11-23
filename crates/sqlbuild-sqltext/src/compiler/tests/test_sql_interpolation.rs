use crate::compiler::models::InterpolationRead;
use crate::compiler::tests::helpers::{interpolated, interpolation_facts, interpolation_host};
use crate::compiler::tests::test_types::{InterpolationFactsTestCase, InterpolationTestCase};

#[test]
fn given_sql_when_interpolating_then_text_and_errors_match_python() {
    let unknown_variable = "unknown project variable '@@missing' in 'sql/models/orders.sql'. \
                            Available vars: größe, layout, region, revision";
    let test_cases = [
        InterpolationTestCase {
            description: "variables in code and quotes; comments and @@@ stay",
            sql: "SELECT @@revision, '@@region' -- @@region\n/* @@region */, @@@window_start, @@1",
            context: false,
            expected_sql: Ok("SELECT 7, 'north' -- @@region\n/* @@region */, @@@window_start, @@1"),
        },
        InterpolationTestCase {
            description: "doubled quotes and backticks",
            sql: "SELECT '@@revision''s', `@@region``@@region`",
            context: false,
            expected_sql: Ok("SELECT '7''s', `north``north`"),
        },
        InterpolationTestCase {
            description: "dollar-quoted text is quoted text",
            sql: "SELECT $tag$ it's $$ -- @@region $tag$, @@region, price$1$ -- @@region",
            context: false,
            expected_sql: Ok("SELECT $tag$ it's $$ -- north $tag$, north, price$1$ -- @@region"),
        },
        InterpolationTestCase {
            description: "environment variables, including non-ASCII names",
            sql: "SELECT '@@ENV:REGION', @@ENV:ÜBER",
            context: false,
            expected_sql: Ok("SELECT 'eu', yes"),
        },
        InterpolationTestCase {
            description: "non-ASCII variable names",
            sql: "SELECT @@größe",
            context: false,
            expected_sql: Ok("SELECT large"),
        },
        InterpolationTestCase {
            description: "context keys take the longest known dotted prefix",
            sql: "GRANT SELECT ON @@CTX:this.schema.@@CTX:this.extra TO reporting",
            context: true,
            expected_sql: Ok("GRANT SELECT ON sales.orders.extra TO reporting"),
        },
        InterpolationTestCase {
            description: "an unknown variable lists the available ones",
            sql: "SELECT '@@missing'",
            context: false,
            expected_sql: Err(unknown_variable),
        },
        InterpolationTestCase {
            description: "a structured variable reports its rendering error",
            sql: "SELECT @@layout",
            context: false,
            expected_sql: Err("SQL variable '@@layout' is an object"),
        },
        InterpolationTestCase {
            description: "a missing environment variable",
            sql: "SELECT @@ENV:SALES_REGION",
            context: false,
            expected_sql: Err(
                "unknown environment variable '@@ENV:SALES_REGION' in 'sql/models/orders.sql'",
            ),
        },
        InterpolationTestCase {
            description: "an empty environment name",
            sql: "SELECT @@ENV:-1",
            context: false,
            expected_sql: Err("invalid environment interpolation token in 'sql/models/orders.sql'"),
        },
        InterpolationTestCase {
            description: "context outside hooks",
            sql: "SELECT @@CTX:this",
            context: false,
            expected_sql: Err("SQL text in 'sql/models/orders.sql' does not allow @@CTX templates"),
        },
        InterpolationTestCase {
            description: "an empty context name",
            sql: "SELECT @@CTX:",
            context: true,
            expected_sql: Err("invalid CTX interpolation token in 'sql/models/orders.sql'"),
        },
        InterpolationTestCase {
            description: "an unknown context key",
            sql: "SELECT @@CTX:that",
            context: true,
            expected_sql: Err("SQL text in 'sql/models/orders.sql' references unknown CTX key 'that'"),
        },
        InterpolationTestCase {
            description: "a context key without a value",
            sql: "SELECT @@CTX:run_id",
            context: true,
            expected_sql: Err(
                "SQL text in 'sql/models/orders.sql' references CTX key 'run_id' but no value is \
                 available",
            ),
        },
        InterpolationTestCase {
            description: "an unclosed quote",
            sql: "SELECT @@revision, 'open",
            context: false,
            expected_sql: Err("SQL interpolation contains an unclosed quoted string"),
        },
        InterpolationTestCase {
            description: "an unclosed dollar quote",
            sql: "SELECT @@region, $$ open",
            context: false,
            expected_sql: Err("SQL interpolation contains an unclosed quoted string"),
        },
        InterpolationTestCase {
            description: "an unclosed block comment",
            sql: "SELECT @@revision /* open",
            context: false,
            expected_sql: Err("SQL interpolation contains an unclosed block comment"),
        },
    ];
    for test_case in test_cases {
        let host = interpolation_host(test_case.context);
        assert_eq!(
            interpolated(&host, test_case.sql),
            test_case
                .expected_sql
                .map(str::to_owned)
                .map_err(str::to_owned),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_tokens_when_interpolating_then_spans_count_code_points_and_reads_keep_order() {
    let host = interpolation_host(true);
    let test_cases = [InterpolationFactsTestCase {
        description: "variables, environment, context and a bare @@ after non-ASCII text",
        sql: "SELECT 'ü', @@größe, @@ENV:REGION, @@CTX:this @@",
        expected_spans: vec![
            [12, 19, 12, 17],
            [21, 33, 19, 21],
            [35, 45, 23, 29],
            [46, 48, 30, 32],
        ],
        expected_reads: vec![
            InterpolationRead::Environment("REGION".to_owned()),
            InterpolationRead::Context("this".to_owned()),
        ],
    }];

    for test_case in test_cases {
        assert_eq!(
            interpolation_facts(&host, test_case.sql),
            (test_case.expected_spans, test_case.expected_reads),
            "{}",
            test_case.description
        );
    }
}
