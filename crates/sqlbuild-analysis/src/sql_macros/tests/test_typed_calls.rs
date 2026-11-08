use crate::sql_macros::main::typed_analysis_sql::typed_analysis_sql;
use crate::sql_macros::main::untyped_call_outputs::untyped_call_outputs;
use crate::sql_macros::models::{ExpandedText, TypedCall};
use crate::sql_macros::tests::helpers::{expanded_text_for, shown_typed_calls, untyped_calls};
use crate::sql_macros::tests::test_types::{
    ExpandedAnalysisTestCase, ReservedNameTestCase, TypedAnalysisTestCase, TypedCallTestCase,
    UntypedOutputsTestCase,
};

const SHOUT: (&str, &str) = (
    "shout",
    "MACRO (description \"Shout\", arguments (value VARCHAR), returns VARCHAR);\n\nUPPER(@value)\n",
);
const ADD: (&str, &str) = (
    "add",
    "MACRO (description \"Add\", arguments (a BIGINT, b BIGINT), returns BIGINT);\n\n@a + @b\n",
);
const LOUD: (&str, &str) = (
    "loud",
    "MACRO (description \"Loud\", arguments (value VARCHAR), returns VARCHAR);\n\n\
     @sql.shout(@value) || '!'\n",
);
const SAME: (&str, &str) = (
    "same",
    "MACRO (description \"Same\", arguments (value VARCHAR), returns VARCHAR);\n\n@value\n",
);
const NOTHING: (&str, &str) = (
    "nothing",
    "MACRO (description \"Nothing\", arguments (value VARCHAR), returns BIGINT);\n\nNULL\n",
);
const PLACES: (&str, &str) = (
    "places",
    "MACRO (description \"Places\", arguments (value BIGINT), returns BIGINT);\n\n\
     @value + @const(\"price_places\")\n",
);
const LIKE_VALUE: (&str, &str) = (
    "like_value",
    "MACRO (description \"Like\", arguments (value ANY, fallback ANY DEFAULT NULL), \
     returns TYPE OF value);\n\nCOALESCE(@value, @fallback)\n",
);

#[test]
fn given_expression_macro_calls_when_expanding_then_each_expansion_is_recorded_where_it_lands() {
    let test_cases = [
        TypedCallTestCase {
            description: "a call in the select list",
            macros: &[SHOUT],
            sql: "SELECT o.id, @sql.shout(o.status) AS s FROM orders AS o",
            expected_calls: &["shout VARCHAR: UPPER(o.status)"],
        },
        TypedCallTestCase {
            description: "a call parenthesised where it stands",
            macros: &[ADD],
            sql: "SELECT @sql.add(o.a, o.b) * 2 AS doubled FROM orders AS o",
            expected_calls: &["add BIGINT: (o.a + o.b)"],
        },
        TypedCallTestCase {
            description: "a nested call in a body, inner first",
            macros: &[SHOUT, LOUD],
            sql: "SELECT @sql.loud(o.status) AS s FROM orders AS o",
            expected_calls: &[
                "shout VARCHAR: UPPER(o.status)",
                "loud VARCHAR: UPPER(o.status) || '!'",
            ],
        },
        TypedCallTestCase {
            description: "a call in another call's argument",
            macros: &[SHOUT],
            sql: "SELECT @sql.shout(@sql.shout(o.status)) AS s FROM orders AS o",
            expected_calls: &[
                "shout VARCHAR: UPPER(o.status)",
                "shout VARCHAR: UPPER(UPPER(o.status))",
            ],
        },
        TypedCallTestCase {
            description: "a body that is its parameter, passed another call",
            macros: &[SHOUT, SAME],
            sql: "SELECT @sql.same(@sql.shout(o.status)) AS s FROM orders AS o",
            expected_calls: &[
                "shout VARCHAR: UPPER(o.status)",
                "same VARCHAR: UPPER(o.status)",
            ],
        },
        TypedCallTestCase {
            description: "calls after rendered declarations",
            macros: &[SHOUT, PLACES],
            sql: "SELECT @const(\"price_places\") AS p, @sql.places(o.a) AS a, \
                  @sql.shout(@enum(\"order_status\").OPEN) AS s FROM orders AS o",
            expected_calls: &["places BIGINT: o.a + 2", "shout VARCHAR: UPPER('open')"],
        },
        TypedCallTestCase {
            description: "a call in a later set-operation branch",
            macros: &[NOTHING],
            sql: "SELECT CAST(NULL AS BIGINT) AS v FROM __ref(\"stg_orders\") AS o\nUNION ALL\n\
                  SELECT @sql.nothing(o.status) AS v FROM __ref(\"stg_orders\") AS o",
            expected_calls: &["nothing BIGINT: NULL"],
        },
        TypedCallTestCase {
            description: "a call inside a Python macro argument is not recorded",
            macros: &[SHOUT],
            sql: "SELECT @py.wrap(@sql.shout(o.status)) AS s FROM orders AS o",
            expected_calls: &[],
        },
    ];
    for test_case in test_cases {
        let expanded = expanded_text_for("duckdb", test_case.macros, test_case.sql)
            .unwrap_or_else(|error| panic!("{}: {error:?}", test_case.description));
        assert_eq!(
            shown_typed_calls(&expanded.sql, &expanded.calls),
            test_case.expected_calls,
            "{}: {}",
            test_case.description,
            expanded.sql
        );
    }
}

#[test]
fn given_typed_calls_when_building_the_analysis_text_then_each_call_with_returns_is_marked() {
    let test_cases = [
        TypedAnalysisTestCase {
            description: "one call",
            sql: "SELECT UPPER(x) AS s FROM t",
            calls: &[(7, 15, Some("VARCHAR"))],
            expected_sql: "SELECT __polyglot_typed(UPPER(x), 'VARCHAR') AS s FROM t",
        },
        TypedAnalysisTestCase {
            description: "a call without returns is not marked",
            sql: "SELECT UPPER(x) AS s FROM t",
            calls: &[(7, 15, None)],
            expected_sql: "SELECT UPPER(x) AS s FROM t",
        },
        TypedAnalysisTestCase {
            description: "nested calls with one range, inner recorded first",
            sql: "SELECT t.x FROM t",
            calls: &[(7, 10, Some("INT")), (7, 10, Some("BIGINT"))],
            expected_sql: "SELECT __polyglot_typed(__polyglot_typed(t.x, 'INT'), 'BIGINT') FROM t",
        },
        TypedAnalysisTestCase {
            description: "adjacent calls close before the next opens",
            sql: "SELECT (t.a)||(t.b) FROM t",
            calls: &[(7, 12, Some("INT")), (14, 19, Some("INT"))],
            expected_sql: "SELECT __polyglot_typed((t.a), 'INT')||__polyglot_typed((t.b), 'INT') FROM t",
        },
        TypedAnalysisTestCase {
            description: "a call ending in a line comment closes it first",
            sql: "SELECT t.x -- note\nFROM t",
            calls: &[(7, 18, Some("DECIMAL(18, 2)"))],
            expected_sql: "SELECT __polyglot_typed(t.x -- note\n, 'DECIMAL(18, 2)')\nFROM t",
        },
        TypedAnalysisTestCase {
            description: "a call in a keyword slot is left unmarked",
            sql: "SELECT EXTRACT(YEAR FROM t.c) AS y FROM t",
            calls: &[(15, 19, Some("VARCHAR"))],
            expected_sql: "SELECT EXTRACT(YEAR FROM t.c) AS y FROM t",
        },
        TypedAnalysisTestCase {
            description: "a bare-word call is never marked",
            sql: "SELECT YEAR AS y FROM t",
            calls: &[(7, 11, Some("VARCHAR"))],
            expected_sql: "SELECT YEAR AS y FROM t",
        },
        TypedAnalysisTestCase {
            description: "a call in a type slot is left unmarked",
            sql: "SELECT CAST(t.x AS DECIMAL(18, 2)) AS y FROM t",
            calls: &[(19, 33, Some("VARCHAR"))],
            expected_sql: "SELECT CAST(t.x AS DECIMAL(18, 2)) AS y FROM t",
        },
        TypedAnalysisTestCase {
            description: "a call whose marker would not parse is left unmarked",
            sql: "SELECT (t.a)(t.b) FROM t",
            calls: &[(7, 12, Some("INT")), (12, 17, Some("INT"))],
            expected_sql: "SELECT (t.a)(t.b) FROM t",
        },
        TypedAnalysisTestCase {
            description: "a quote in returns is doubled",
            sql: "SELECT t.x FROM t",
            calls: &[(7, 10, Some("ENUM('a')"))],
            expected_sql: "SELECT __polyglot_typed(t.x, 'ENUM(''a'')') FROM t",
        },
    ];
    for test_case in test_cases {
        let calls: Vec<TypedCall> = test_case
            .calls
            .iter()
            .map(|(start, end, returns)| TypedCall {
                returns_like: None,
                start: *start,
                end: *end,
                macro_name: "m".to_owned(),
                returns: returns.map(str::to_owned),
            })
            .collect();
        let (typed, insertions) = typed_analysis_sql(test_case.sql, &calls, "duckdb");
        assert_eq!(typed, test_case.expected_sql, "{}", test_case.description);
        let removed: String = {
            let mut text: String = String::new();
            let mut cursor: usize = 0;
            for insertion in &insertions {
                text.push_str(&typed[cursor..insertion.output.0]);
                cursor = insertion.output.1;
            }
            text.push_str(&typed[cursor..]);
            text
        };
        assert_eq!(removed, test_case.sql, "{}", test_case.description);
    }
}

#[test]
fn given_the_reserved_analysis_function_when_comparing_then_authored_sql_rejects_the_parsed_name() {
    let test_cases = [ReservedNameTestCase {
        description: "the typed marker",
        authored_name:
            sqlbuild_attachments::cursor_intrinsics::constants::RESERVED_ANALYSIS_FUNCTION,
        expected_parsed_name: sqlquack_syntax::expressions::tree::constants::TYPED_EXPRESSION_FUNCTION,
    }];

    for test_case in test_cases {
        assert_eq!(
            test_case.authored_name, test_case.expected_parsed_name,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_calls_without_returns_when_listing_outputs_then_only_whole_named_items_are_named() {
    let test_cases = [
        UntypedOutputsTestCase {
            description: "aliased whole calls are named, operands and unnamed items are not",
            sql: "SELECT UPPER(a) AS loud, UPPER(b) quiet, UPPER(c) || 'x' AS joined, UPPER(d) FROM t",
            call_texts: &["UPPER(a)", "UPPER(b)", "UPPER(c)", "UPPER(d)"],
            expected_outputs: &[("loud", "shout"), ("quiet", "shout")],
        },
        UntypedOutputsTestCase {
            description: "a keyword after a call is not its alias",
            sql: "SELECT id, UPPER(a) FROM t",
            call_texts: &["UPPER(a)"],
            expected_outputs: &[],
        },
    ];

    for test_case in test_cases {
        let calls: Vec<TypedCall> = untyped_calls(test_case.sql, test_case.call_texts);

        let outputs: Vec<(String, String)> = untyped_call_outputs(test_case.sql, &calls);

        let named: Vec<(&str, &str)> = outputs
            .iter()
            .map(|(output, name)| (output.as_str(), name.as_str()))
            .collect();
        assert_eq!(
            named, test_case.expected_outputs,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_returns_type_of_call_when_building_the_analysis_text_then_it_asserts_its_argument() {
    let test_cases = [ExpandedAnalysisTestCase {
        description: "the call is typed like its argument",
        macros: &[LIKE_VALUE],
        sql: "SELECT @sql.like_value(o.created_at + 1) AS v FROM orders AS o",
        expected_sql: "SELECT __polyglot_typed_like(COALESCE((o.created_at + 1), NULL), \
                       o.created_at + 1) AS v FROM orders AS o",
    }];

    for test_case in test_cases {
        let expanded: ExpandedText =
            expanded_text_for("duckdb", test_case.macros, test_case.sql).expect("expands");

        let (typed, _) = typed_analysis_sql(&expanded.sql, &expanded.calls, "duckdb");

        assert_eq!(typed, test_case.expected_sql, "{}", test_case.description);
    }
}
