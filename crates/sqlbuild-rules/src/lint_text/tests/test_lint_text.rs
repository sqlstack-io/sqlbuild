use crate::lint_text::_helpers::format_bodies::unwrapped_expression;
use crate::lint_text::_helpers::sites::site_pattern;
use crate::lint_text::_helpers::sql_diagnostics::interpolation_text_at;
use crate::lint_text::constants::SITE_START_BYTES;
use crate::lint_text::errors::LintTextError;
use crate::lint_text::main::format_sql_bodies::format_sql_bodies;
use crate::lint_text::main::frame_sql_macro_expression::frame_sql_macro_expression;
use crate::lint_text::main::lint_sql_bodies::lint_sql_bodies;
use crate::lint_text::main::neutralize_audit_parameters::neutralize_audit_parameters;
use crate::lint_text::main::neutralize_context_interpolation::neutralize_context_interpolation;
use crate::lint_text::main::neutralize_interpolation::neutralize_interpolation;
use crate::lint_text::main::neutralize_sql_macro_parameters::neutralize_sql_macro_parameters;
use crate::lint_text::main::neutralize_test_parameters::neutralize_test_parameters;
use crate::lint_text::main::reject_inline_suppressions::reject_inline_suppressions;
use crate::lint_text::main::restore_interpolation::restore_interpolation;
use crate::lint_text::models::{ExpansionSpan, FormatFile, LintSqlBody, LintSqlOptions};
use crate::lint_text::tests::helpers::directive_finding;
use crate::lint_text::tests::test_types::{
    Finding, FormatBodiesTestCase, LintBodiesTestCase, NeutralizeTestCase, ParameterTestCase,
    RestoreTestCase, SitePatternTestCase, SlotReadTestCase, SuppressionTestCase, TokenAtTestCase,
    UnwrappedExpressionTestCase,
};
use sqlbuild_analysis::sql_macros::main::parse_sql_macro::parse_sql_macro;

const SUPPRESSED_FILE: &str =
    "-- sqb: ignore SQLCLEAN005 because kept\n\n-- note\nselect 1\n-- sqb: ignore nope\nselect 2\n";
const CROSS_JOIN_FILE: &str = "select o.order_id\nfrom orders as o\n-- sqb: ignore SQLCLEAN038 because every order meets every policy\ncross join __seed(\"order_policy\") as p\n";

#[test]
fn given_site_patterns_when_text_opens_with_another_byte_then_no_site_starts_there()
-> Result<(), LintTextError> {
    let test_cases = [
        SitePatternTestCase {
            description: "lint sites",
            formatting: false,
            expected_other_start_bytes: Vec::new(),
        },
        SitePatternTestCase {
            description: "format sites",
            formatting: true,
            expected_other_start_bytes: Vec::new(),
        },
    ];
    for test_case in test_cases {
        let pattern = site_pattern(test_case.formatting)?;
        let other_start_bytes: Vec<u8> = (0..=127u8)
            .filter(|byte| !SITE_START_BYTES.contains(byte))
            .filter(|byte| {
                ["@@x", "@x", "${x}", "__ref(", "__REF (", "__udf("]
                    .iter()
                    .any(|site| pattern.is_match(&format!("{}{site}", char::from(*byte))))
            })
            .collect();
        assert_eq!(
            other_start_bytes, test_case.expected_other_start_bytes,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_authored_sql_when_neutralizing_then_sites_use_character_offsets()
-> Result<(), LintTextError> {
    let test_cases = [
        NeutralizeTestCase {
            description: "reference, macro call and template",
            body: "select @py.m(a, ')') from __ref('o') where x > ${v}",
            formatting: false,
            expected_text: "select __sqlclean_interpolation_0__ from __sqlclean_interpolation_1__ where x > __sqlclean_interpolation_2__",
            expected_sites: vec![
                ("@py.m(a, ')')", 7, 20),
                ("__ref('o')", 26, 36),
                ("${v}", 47, 51),
            ],
        },
        NeutralizeTestCase {
            description: "non-ASCII text before a site shifts character offsets only",
            body: "select 'é' as n, @@CTX:run.id",
            formatting: false,
            expected_text: "select 'é' as n, __sqlclean_interpolation_0__",
            expected_sites: vec![("@@CTX:run.id", 17, 29)],
        },
        NeutralizeTestCase {
            description: "a quoted argument stands as a string, a raw one as a name",
            body: "comment on table @relation is @'comment'",
            formatting: false,
            expected_text: "comment on table __sqlclean_interpolation_0__ is '__sqlclean_interpolation_1__'",
            expected_sites: vec![("@relation", 17, 26), ("@'comment'", 30, 40)],
        },
        NeutralizeTestCase {
            description: "comments and strings hide sites",
            body: "-- @x\nselect '@y', \"__ref(z)\" /* ${w} */",
            formatting: false,
            expected_text: "-- @x\nselect '@y', \"__ref(z)\" /* ${w} */",
            expected_sites: vec![],
        },
        NeutralizeTestCase {
            description: "lint leaves spaced and upper-case intrinsics alone",
            body: "select __REF('a'), __ref ('b'), __cursor_start()",
            formatting: false,
            expected_text: "select __REF('a'), __ref ('b'), __cursor_start()",
            expected_sites: vec![],
        },
        NeutralizeTestCase {
            description: "formatting protects every intrinsic spelling",
            body: "select __REF('a'), __ref ('b'), __cursor_start()",
            formatting: true,
            expected_text: "select __sqlclean_interpolation_0__, __sqlclean_interpolation_1__, __sqlclean_interpolation_2__",
            expected_sites: vec![
                ("__REF('a')", 7, 17),
                ("__ref ('b')", 19, 30),
                ("__cursor_start()", 32, 48),
            ],
        },
    ];

    for test_case in test_cases {
        let neutralized = neutralize_interpolation(test_case.body, "duckdb", test_case.formatting)?;
        let sites: Vec<(&str, usize, usize)> = neutralized
            .sites
            .iter()
            .map(|site| {
                (
                    site.original_text.as_str(),
                    site.original_start,
                    site.original_end,
                )
            })
            .collect();

        assert_eq!(
            neutralized.text, test_case.expected_text,
            "{}",
            test_case.description
        );
        assert_eq!(sites, test_case.expected_sites, "{}", test_case.description);
    }
    Ok(())
}

#[test]
fn given_generated_parameters_when_neutralizing_then_only_bare_parameters_are_replaced() {
    let test_cases = [
        ParameterTestCase {
            description: "quoted and raw audit parameters, but not calls or doubled at-signs",
            body: "select * from @this where @column < 0 and @'kind' = @py.f(1) and @@x",
            neutralize: neutralize_audit_parameters,
            expected_text: "select * from __sqlclean_relation_parameter_0__ where __sqlclean_relation_parameter_1__ < 0 and '__sqlclean_relation_parameter_2__' = @py.f(1) and @@x",
        },
        ParameterTestCase {
            description: "SQL macro parameters followed by words and a parenthesis",
            body: "@value IS NULL OR TRIM(@value) = ''",
            neutralize: |body| neutralize_sql_macro_parameters(body, &[]),
            expected_text: "__sqlclean_macro_parameter_0__ IS NULL OR TRIM(__sqlclean_macro_parameter_1__) = ''",
        },
        ParameterTestCase {
            description: "SQL macro parameters before IN and a parenthesised predicate",
            body: "CASE WHEN @x IN ('a', 'b') THEN 1 END + CASE WHEN @x IS NULL AND (@y IS NULL) THEN 1 END",
            neutralize: |body| neutralize_sql_macro_parameters(body, &[]),
            expected_text: "CASE WHEN __sqlclean_macro_parameter_0__ IN ('a', 'b') THEN 1 END + CASE WHEN __sqlclean_macro_parameter_1__ IS NULL AND (__sqlclean_macro_parameter_2__ IS NULL) THEN 1 END",
        },
        ParameterTestCase {
            description: "SQL macro slot reads hide the slot name, whole after an alias",
            body: "SELECT s.@src.amount AS amount, @src.id, @src.* FROM @src AS s",
            neutralize: |body| neutralize_sql_macro_parameters(body, &[]),
            expected_text: "SELECT s.__sqlclean_macro_parameter_0__ AS amount, __sqlclean_macro_parameter_1__.__sqlclean_macro_parameter_2__, __sqlclean_macro_parameter_3__.* FROM __sqlclean_macro_parameter_4__ AS s",
        },
        ParameterTestCase {
            description: "SQL macro calls, enum references and quoted text keep their at-signs",
            body: "@sql.other(@v) + @enum(\"s\").A + LENGTH('@v') + @@x",
            neutralize: |body| neutralize_sql_macro_parameters(body, &[]),
            expected_text: "@sql.other(__sqlclean_macro_parameter_0__) + @enum(\"s\").A + LENGTH('@v') + @@x",
        },
        ParameterTestCase {
            description: "SQL test parameters, spaced or not, but not a malformed reference",
            body: "SELECT @param(\"a\") AS a, @param ( \"b\" ) AS b, @param(c) AS c",
            neutralize: neutralize_test_parameters,
            expected_text: "SELECT '__sqlbuild_test_parameter_0__' AS a, '__sqlbuild_test_parameter_1__' AS b, @param(c) AS c",
        },
        ParameterTestCase {
            description: "context interpolation in hooks",
            body: "analyze @@CTX:relation; select @@CTX:a.b",
            neutralize: neutralize_context_interpolation,
            expected_text: "analyze __sqlclean_value_parameter_0__; select __sqlclean_value_parameter_1__",
        },
    ];

    for test_case in test_cases {
        let neutralized = (test_case.neutralize)(test_case.body);

        assert_eq!(
            neutralized.text, test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_relation_slot_reads_when_neutralizing_then_they_stand_as_their_expansion_does() {
    let header: &str = "MACRO (\n  description \"Weekly rates\",\n  \
         arguments (weekly RELATION (keys COLUMNS, refund_count BIGINT), cutoff BIGINT),\n);\n\n";
    let test_cases = [
        SlotReadTestCase {
            description: "a plain group read is one bare value",
            body: "SELECT @weekly.keys, orders FROM counted GROUP BY @weekly.keys",
            expected_text: "SELECT __sqlclean_macro_parameter_0__, orders FROM counted GROUP BY __sqlclean_macro_parameter_1__",
        },
        SlotReadTestCase {
            description: "plain single-slot and undeclared reads go through the one relation",
            body: "SELECT @weekly.refund_count, @weekly.week FROM @weekly WHERE @cutoff > @cutoff",
            expected_text: "SELECT __sqlclean_macro_parameter_0__.__sqlclean_macro_parameter_1__, __sqlclean_macro_parameter_0__.__sqlclean_macro_parameter_2__ FROM __sqlclean_macro_parameter_0__ WHERE __sqlclean_macro_parameter_3__ > __sqlclean_macro_parameter_4__",
        },
        SlotReadTestCase {
            description: "aliased reads stay whole after their alias",
            body: "SELECT s.@weekly.keys, s.@weekly.refund_count FROM @weekly AS s",
            expected_text: "SELECT s.__sqlclean_macro_parameter_0__, s.__sqlclean_macro_parameter_1__ FROM __sqlclean_macro_parameter_2__ AS s",
        },
    ];

    for test_case in test_cases {
        let source: String = format!("{header}{}\n", test_case.body);
        let parameters = parse_sql_macro("weekly_rates", "macros/sql/weekly_rates.sql", &source)
            .map(|definition| definition.parameters)
            .unwrap_or_default();

        assert_eq!(
            neutralize_sql_macro_parameters(test_case.body, &parameters).text,
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_formatted_sql_when_restoring_then_each_sentinel_must_appear_once()
-> Result<(), LintTextError> {
    let test_cases = [
        RestoreTestCase {
            description: "sentinels in any case restore to the original text",
            body: "select @m, ${v}",
            fixed: "SELECT __SQLCLEAN_INTERPOLATION_0__, __sqlclean_interpolation_1__",
            expected_restored: Ok("SELECT @m, ${v}".to_string()),
        },
        RestoreTestCase {
            description: "a duplicated sentinel is ambiguous",
            body: "select @m",
            fixed: "select __sqlclean_interpolation_0__, __SQLCLEAN_INTERPOLATION_0__",
            expected_restored: Err("formatted SQL contains 2 occurrences of sentinel '__sqlclean_interpolation_0__' standing in for '@m'; expected exactly 1".to_string()),
        },
    ];

    for test_case in test_cases {
        let neutralized = neutralize_interpolation(test_case.body, "duckdb", true)?;

        let restored = restore_interpolation(test_case.fixed, &neutralized.sites)
            .map_err(|error| error.to_string());

        assert_eq!(
            restored, test_case.expected_restored,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_character_offset_when_reading_token_then_only_site_starts_have_tokens() {
    let test_cases = [
        TokenAtTestCase {
            description: "a macro call after a multibyte character",
            body: "é @py.m(x) y",
            start: 2,
            expected_token: Some("@py.m(x)"),
        },
        TokenAtTestCase {
            description: "a plain character",
            body: "é @py.m(x) y",
            start: 0,
            expected_token: None,
        },
        TokenAtTestCase {
            description: "a spaced intrinsic call",
            body: "x __source ('a', 'b')",
            start: 2,
            expected_token: Some("__source ('a', 'b')"),
        },
    ];

    for test_case in test_cases {
        let token = interpolation_text_at(test_case.body, test_case.start, "duckdb");

        assert_eq!(
            token.as_deref(),
            test_case.expected_token,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_inline_directives_when_rejecting_then_each_prints_its_configured_exception()
-> Result<(), LintTextError> {
    let test_cases = [
        SuppressionTestCase {
            description: "a reasoned directive and one without a reason",
            contents: SUPPRESSED_FILE,
            expected_findings: vec![
                directive_finding(
                    1,
                    "SQLCLEAN005",
                    &[
                        "rule = \"SQLCLEAN005\"",
                        "path = \"models/orders.sql\"",
                        "reason = \"kept\"",
                    ],
                ),
                directive_finding(
                    5,
                    "nope",
                    &[
                        "rule = \"nope\"",
                        "path = \"models/orders.sql\"",
                        "reason = \"<why this deviation is intentional>\"",
                    ],
                ),
            ],
        },
        SuppressionTestCase {
            description: "a cross join directive names the joined dependency",
            contents: CROSS_JOIN_FILE,
            expected_findings: vec![directive_finding(
                3,
                "SQLCLEAN038",
                &[
                    "rule = \"SQLCLEAN038\"",
                    "path = \"models/orders.sql\"",
                    "relation = \"order_policy\"",
                    "reason = \"every order meets every policy\"",
                ],
            )],
        },
        SuppressionTestCase {
            description: "a string literal mentioning a directive is not one",
            contents: "select '-- sqb: ignore SQLCLEAN004 because text' as note, 'sqb: ignore' as other\n",
            expected_findings: vec![],
        },
        SuppressionTestCase {
            description: "a trailing directive after code names the join on its line",
            contents: "select o.order_id\nfrom orders as o\ncross join customers as c -- sqb: ignore SQLCLEAN038 because deliberate\n",
            expected_findings: vec![directive_finding(
                3,
                "SQLCLEAN038",
                &[
                    "rule = \"SQLCLEAN038\"",
                    "path = \"models/orders.sql\"",
                    "relation = \"customers\"",
                    "reason = \"deliberate\"",
                ],
            )],
        },
        SuppressionTestCase {
            description: "a directive in a block comment",
            contents: "select 1\n/* sqb: ignore SQLCLEAN005 because kept */\nselect 2\n",
            expected_findings: vec![directive_finding(
                2,
                "SQLCLEAN005",
                &[
                    "rule = \"SQLCLEAN005\"",
                    "path = \"models/orders.sql\"",
                    "reason = \"kept\"",
                ],
            )],
        },
        SuppressionTestCase {
            description: "a file without directives",
            contents: "select 1\n-- a plain comment\n",
            expected_findings: vec![],
        },
    ];

    for test_case in test_cases {
        let findings = reject_inline_suppressions(&[("models/orders.sql", test_case.contents)])?;

        assert_eq!(
            findings, test_case.expected_findings,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_sql_files_when_formatting_bodies_then_sites_and_fixture_ceremony_survive()
-> Result<(), LintTextError> {
    let test_cases = [
        FormatBodiesTestCase {
            description: "a model body with a reference and a macro",
            dialect: "duckdb",
            relative_parts: Some(vec!["models".to_string(), "orders.sql".to_string()]),
            contents: "MODEL (description \"d\");\nselect a, @py.m('x') as m from __ref('o')\n",
            expected_formatted: vec![(
                0,
                "MODEL (description \"d\");\nSELECT\n  a,\n  @py.m('x') AS m\nFROM __ref('o')\n"
                    .to_string(),
            )],
            expected_fault_lines: vec![],
        },
        FormatBodiesTestCase {
            description: "a fixture body keeps its omitted ceremonial select",
            dialect: "duckdb",
            relative_parts: Some(vec![
                "tests".to_string(),
                "unit".to_string(),
                "t.sql".to_string(),
            ]),
            contents: "TEST();\nwith __ref__o as (select 1 as a)\n",
            expected_formatted: vec![(
                0,
                "TEST();\nWITH __ref__o AS (\n  SELECT 1 AS a\n)\n".to_string(),
            )],
            expected_fault_lines: vec![],
        },
        FormatBodiesTestCase {
            description: "a scalar function's expression body parses as a SELECT item",
            dialect: "snowflake",
            relative_parts: Some(vec![
                "functions".to_string(),
                "sql".to_string(),
                "next_quantity.sql".to_string(),
            ]),
            contents: "FUNCTION (arguments (quantity INTEGER), returns INTEGER);\n\nquantity + 1\n",
            expected_formatted: vec![],
            expected_fault_lines: vec![],
        },
        FormatBodiesTestCase {
            description: "a table function's query body formats as written",
            dialect: "snowflake",
            relative_parts: Some(vec![
                "functions".to_string(),
                "sql".to_string(),
                "open_orders.sql".to_string(),
            ]),
            contents: "FUNCTION (returns table (a INTEGER));\n\nselect a from t\n",
            expected_formatted: vec![(
                0,
                "FUNCTION (returns table (a INTEGER));\n\nSELECT a\nFROM t\n".to_string(),
            )],
            expected_fault_lines: vec![],
        },
        FormatBodiesTestCase {
            description: "a table function's VALUES body stays a query",
            dialect: "duckdb",
            relative_parts: Some(vec![
                "functions".to_string(),
                "sql".to_string(),
                "pairs.sql".to_string(),
            ]),
            contents: "FUNCTION (returns table (a INTEGER));\n\nVALUES (1), (2)\n",
            expected_formatted: vec![],
            expected_fault_lines: vec![],
        },
        FormatBodiesTestCase {
            description: "a table function's parenthesized query keeps its layout",
            dialect: "snowflake",
            relative_parts: Some(vec![
                "functions".to_string(),
                "sql".to_string(),
                "nested_orders.sql".to_string(),
            ]),
            contents: "FUNCTION (returns table (a INTEGER));\n\n(\n  SELECT a\n  FROM t\n)\n",
            expected_formatted: vec![],
            expected_fault_lines: vec![],
        },
        FormatBodiesTestCase {
            description: "a scalar function body after a comment keeps its column",
            dialect: "duckdb",
            relative_parts: Some(vec![
                "functions".to_string(),
                "sql".to_string(),
                "bounded_quantity.sql".to_string(),
            ]),
            contents: "FUNCTION (arguments (q INTEGER), returns INTEGER);\n\n-- note\nCASE WHEN q > 1 THEN q ELSE 0 END\n",
            expected_formatted: vec![],
            expected_fault_lines: vec![],
        },
        FormatBodiesTestCase {
            description: "a quoted hook argument stands as a string",
            dialect: "snowflake",
            relative_parts: Some(vec![
                "hooks".to_string(),
                "sql".to_string(),
                "comment.sql".to_string(),
            ]),
            contents: "HOOK ();\n\ncomment on table @relation is @'comment'\n",
            expected_formatted: vec![(
                0,
                "HOOK ();\n\nCOMMENT ON TABLE @relation IS @'comment'\n".to_string(),
            )],
            expected_fault_lines: vec![],
        },
        FormatBodiesTestCase {
            description: "an unparseable body is a fault on its first line",
            dialect: "duckdb",
            relative_parts: None,
            contents: "MODEL ();\n\nSELECT FROM WHERE (((\n",
            expected_formatted: vec![],
            expected_fault_lines: vec![3],
        },
    ];

    for test_case in test_cases {
        let files = [FormatFile {
            relative_parts: test_case.relative_parts.clone(),
            contents: test_case.contents.to_string(),
        }];

        let result = format_sql_bodies(&files, test_case.dialect, 100)?;
        let fault_lines: Vec<usize> = result.faults.iter().map(|fault| fault.line).collect();

        assert_eq!(
            result.formatted, test_case.expected_formatted,
            "{}",
            test_case.description
        );
        assert_eq!(
            fault_lines, test_case.expected_fault_lines,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_expanded_bodies_when_linting_then_findings_map_to_authored_lines() {
    let test_cases = [
        LintBodiesTestCase {
            description: "an unused CTE after a header maps to its authored line",
            contents: "MODEL ();\nwith dead as (select 1 as y), live as (select 1 as a)\nselect a from live\n",
            body: LintSqlBody {
                body_start: 10,
                lint_text:
                    "with dead as (select 1 as y), live as (select 1 as a)\nselect a from live"
                        .to_string(),
                ..LintSqlBody::default()
            },
            expected_findings: vec![("SQLCLEAN005", 2, 6, Some((2, 10)))],
        },
        LintBodiesTestCase {
            description: "a parse failure is a fault at the mapped parser position",
            contents: "MODEL ();\n\nselect a from from t\n",
            body: LintSqlBody {
                body_start: 11,
                lint_text: "select a from from t".to_string(),
                ..LintSqlBody::default()
            },
            expected_findings: vec![("L003", 3, 19, None)],
        },
        LintBodiesTestCase {
            description: "a finding inside generated SQL points at the interpolation token",
            contents: "select a from t where @py.isnull()\n",
            body: LintSqlBody {
                lint_text: "select a from t where a = null".to_string(),
                passes: vec![vec![ExpansionSpan {
                    source_start: 22,
                    source_end: 34,
                    output_start: 22,
                    output_end: 30,
                }]],
                ..LintSqlBody::default()
            },
            expected_findings: vec![("SQLCLEAN001", 1, 23, Some((1, 35)))],
        },
    ];

    for test_case in test_cases {
        let options = LintSqlOptions {
            dialect: "duckdb".to_string(),
            enabled_rules: Some(vec!["SQLCLEAN001".to_string(), "SQLCLEAN005".to_string()]),
            max_literal_length: 100,
            max_ranking_order_by: 5,
            ..LintSqlOptions::default()
        };

        let result = lint_sql_bodies(&[test_case.body], &[test_case.contents], &options);
        let findings: Vec<Finding<'_>> = result
            .iter()
            .map(|finding| {
                (
                    finding.code.as_str(),
                    finding.line,
                    finding.column,
                    finding.end,
                )
            })
            .collect();

        assert_eq!(
            findings, test_case.expected_findings,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_formatted_expression_select_when_unwrapping_then_strings_keep_their_lines() {
    let test_cases = [
        UnwrappedExpressionTestCase {
            description: "the expression is dedented one level",
            formatted: "SELECT\n  ROUND(\n    @v,\n    2\n  )",
            expected_expression: Some("ROUND(\n  @v,\n  2\n)"),
        },
        UnwrappedExpressionTestCase {
            description: "lines inside a multi-line string literal are not dedented",
            formatted: "SELECT\n  CONCAT(\n    'first\n    second',\n    @v\n  )",
            expected_expression: Some("CONCAT(\n  'first\n    second',\n  @v\n)"),
        },
        UnwrappedExpressionTestCase {
            description: "text that is not a select is not an expression",
            formatted: "WITH a AS (SELECT 1)",
            expected_expression: None,
        },
    ];

    for test_case in test_cases {
        let unwrapped: Option<String> = unwrapped_expression(test_case.formatted);

        assert_eq!(
            unwrapped.as_deref(),
            test_case.expected_expression,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sql_macro_bodies_when_linting_then_frames_and_generated_comments_are_not_judged() {
    let parenthesised = frame_sql_macro_expression("((x) IS NULL AND (x) IS NULL) OR TRIM(x) = ''");
    let null_comparison = frame_sql_macro_expression("x = NULL OR y;\n");
    let test_cases = [
        LintBodiesTestCase {
            description: "an expression body opening with two parentheses parses as a SELECT",
            contents: "((x) IS NULL AND (x) IS NULL) OR TRIM(x) = ''\n",
            body: LintSqlBody {
                lint_text: parenthesised.text,
                passes: vec![parenthesised.pass],
                expression_frame: Some(parenthesised.frame),
                ..LintSqlBody::default()
            },
            expected_findings: vec![],
        },
        LintBodiesTestCase {
            description: "a finding in a framed expression maps to the body, none to the frame",
            contents: "x = NULL OR y;\n",
            body: LintSqlBody {
                lint_text: null_comparison.text,
                passes: vec![null_comparison.pass],
                expression_frame: Some(null_comparison.frame),
                ..LintSqlBody::default()
            },
            expected_findings: vec![("SQLCLEAN001", 1, 3, Some((1, 4)))],
        },
        LintBodiesTestCase {
            description: "comments a macro expansion generated are not judged in the caller",
            contents: "SELECT a, NOT @sql.m(b) AS c FROM t\n",
            body: LintSqlBody {
                lint_text: "SELECT a, NOT -- why\nCONTAINS(b, /* sign */ '@') AS c FROM t"
                    .to_string(),
                passes: vec![vec![ExpansionSpan {
                    source_start: 14,
                    source_end: 23,
                    output_start: 14,
                    output_end: 50,
                }]],
                ..LintSqlBody::default()
            },
            expected_findings: vec![("SQLCLEAN034", 1, 1, Some((1, 7)))],
        },
        LintBodiesTestCase {
            description: "comments a Python macro generated are judged in the caller",
            contents: "SELECT a, NOT @py.m(b)  AS c FROM t\n",
            body: LintSqlBody {
                lint_text: "SELECT a, NOT -- why\nCONTAINS(b, /* sign */ '@') AS c FROM t"
                    .to_string(),
                passes: vec![vec![ExpansionSpan {
                    source_start: 14,
                    source_end: 23,
                    output_start: 14,
                    output_end: 50,
                }]],
                ..LintSqlBody::default()
            },
            expected_findings: vec![
                ("SQLCLEAN034", 1, 1, Some((1, 7))),
                ("SQLCLEAN033", 1, 11, Some((1, 14))),
                ("SQLCLEAN033", 1, 15, Some((1, 23))),
            ],
        },
        LintBodiesTestCase {
            description: "the same comments authored in the caller are judged",
            contents: "SELECT a, NOT -- why\nCONTAINS(b, /* sign */ '@') AS c FROM t\n",
            body: LintSqlBody {
                lint_text: "SELECT a, NOT -- why\nCONTAINS(b, /* sign */ '@') AS c FROM t"
                    .to_string(),
                ..LintSqlBody::default()
            },
            expected_findings: vec![
                ("SQLCLEAN034", 1, 1, Some((1, 7))),
                ("SQLCLEAN033", 1, 11, Some((1, 14))),
                ("SQLCLEAN033", 2, 11, Some((2, 12))),
            ],
        },
    ];

    for test_case in test_cases {
        let options = LintSqlOptions {
            dialect: "duckdb".to_string(),
            enabled_rules: Some(
                ["SQLCLEAN001", "SQLCLEAN022", "SQLCLEAN033", "SQLCLEAN034"]
                    .map(str::to_string)
                    .to_vec(),
            ),
            max_literal_length: 100,
            max_ranking_order_by: 5,
            ..LintSqlOptions::default()
        };

        let result = lint_sql_bodies(&[test_case.body], &[test_case.contents], &options);
        let findings: Vec<Finding<'_>> = result
            .iter()
            .map(|finding| {
                (
                    finding.code.as_str(),
                    finding.line,
                    finding.column,
                    finding.end,
                )
            })
            .collect();

        assert_eq!(
            findings, test_case.expected_findings,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_snowflake_table_function_calls_when_formatting_then_each_call_is_kept_whole()
-> Result<(), LintTextError> {
    let test_cases = [
        FormatBodiesTestCase {
            description: "a call outside TABLE( ) stands as one relation",
            dialect: "snowflake",
            relative_parts: None,
            contents: "MODEL ();\nselect l.a from __table_fn(\"f\")('B-1', 2) as l\n",
            expected_formatted: vec![(
                0,
                "MODEL ();\nSELECT l.a\nFROM __table_fn(\"f\")('B-1', 2) AS l\n".to_string(),
            )],
            expected_fault_lines: vec![],
        },
        FormatBodiesTestCase {
            description: "a call the author wrapped in TABLE( ) keeps its arguments",
            dialect: "snowflake",
            relative_parts: None,
            contents: "MODEL ();\nselect a from table(__table_fn(\"f\")( 42 ))\n",
            expected_formatted: vec![(
                0,
                "MODEL ();\nSELECT a\nFROM TABLE(__table_fn(\"f\")(42))\n".to_string(),
            )],
            expected_fault_lines: vec![],
        },
        FormatBodiesTestCase {
            description: "a call after a comma LATERAL keeps its arguments",
            dialect: "snowflake",
            relative_parts: None,
            contents: "MODEL ();\nselect t.a, l.value from t, lateral __table_fn(\"f\")(t.x) as l\n",
            expected_formatted: vec![(
                0,
                "MODEL ();\nSELECT\n  t.a,\n  l.value\nFROM t, LATERAL __table_fn(\"f\")(t.x) AS l\n"
                    .to_string(),
            )],
            expected_fault_lines: vec![],
        },
        FormatBodiesTestCase {
            description: "a call after JOIN LATERAL keeps its arguments",
            dialect: "snowflake",
            relative_parts: None,
            contents: "MODEL ();\nselect t.a from t join lateral __table_fn(\"f\")(t.x) as l on true\n",
            expected_formatted: vec![(
                0,
                "MODEL ();\nSELECT t.a\nFROM t\nJOIN LATERAL __table_fn(\"f\")(t.x) AS l\n  ON TRUE\n"
                    .to_string(),
            )],
            expected_fault_lines: vec![],
        },
    ];

    for test_case in test_cases {
        let files = [FormatFile {
            relative_parts: test_case.relative_parts.clone(),
            contents: test_case.contents.to_string(),
        }];

        let result = format_sql_bodies(&files, test_case.dialect, 100)?;
        let fault_lines: Vec<usize> = result.faults.iter().map(|fault| fault.line).collect();

        assert_eq!(
            (result.formatted, fault_lines),
            (test_case.expected_formatted, test_case.expected_fault_lines),
            "{}",
            test_case.description
        );
    }
    Ok(())
}
