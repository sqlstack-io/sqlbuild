use crate::sql_references::main::extract_sql_references::extract_sql_references;
use crate::sql_references::tests::helpers::{
    backslash_hash_comments, dbt_reference, extracted, failed, generic, invalid_call,
    partitioned_table_function, reference, rejected, table_function,
};
use crate::sql_references::tests::test_types::ExtractSqlReferencesTestCase;

#[test]
fn given_sql_when_extracting_references_then_matches_python_scanner() {
    let test_cases = [
        ExtractSqlReferencesTestCase {
            description: "every kind in authored order",
            sql: "SELECT * FROM __ref(\"orders\") JOIN __source(\"raw_orders\") \
                  JOIN __seed(\"regions\") JOIN __dbt_ref(\"shop\" ,\u{3000}\"customers\") \
                  WHERE __udf(\"clean\")(x) > 0 OR __dbt_ref(\"orders\") IS NULL",
            syntax: generic(),
            expected_extraction: extracted(vec![
                reference("ref", "orders"),
                reference("source", "raw_orders"),
                reference("seed", "regions"),
                dbt_reference("shop", "customers"),
                reference("udf", "clean"),
                reference("dbt_ref", "orders"),
            ]),
        },
        ExtractSqlReferencesTestCase {
            description: "table function counts top-level call arguments",
            sql: "SELECT * FROM __table_fn(\"expand_orders\") ( 1, f(2, 3), 'a,b' /* , */ )",
            syntax: generic(),
            expected_extraction: extracted(vec![table_function("expand_orders", 3)]),
        },
        ExtractSqlReferencesTestCase {
            description: "a partition clause after the call marks it partitioned",
            sql: "SELECT s.* FROM r, __table_fn(\"smooth\")(r.a, r.b)\n  over ( partition\tby r.device_id ORDER BY r.ts) AS s",
            syntax: generic(),
            expected_extraction: extracted(vec![partitioned_table_function("smooth", 2)]),
        },
        ExtractSqlReferencesTestCase {
            description: "an empty window or a following OVER word is not a partition clause",
            sql: "SELECT * FROM __table_fn(\"a\")(1) OVER () , __table_fn(\"b\")(2) overflow (partition by x)",
            syntax: generic(),
            expected_extraction: extracted(vec![table_function("a", 1), table_function("b", 1)]),
        },
        ExtractSqlReferencesTestCase {
            description: "table function without call arguments",
            sql: "SELECT * FROM __table_fn(\"daily_orders\")()",
            syntax: generic(),
            expected_extraction: extracted(vec![table_function("daily_orders", 0)]),
        },
        ExtractSqlReferencesTestCase {
            description: "references inside table function arguments are scanned",
            sql: "SELECT * FROM __table_fn(\"f\")((SELECT id FROM __ref(\"orders\")))",
            syntax: generic(),
            expected_extraction: extracted(vec![
                table_function("f", 1),
                reference("ref", "orders"),
            ]),
        },
        ExtractSqlReferencesTestCase {
            description: "comment inside a reference call is rejected",
            sql: "SELECT * FROM __ref(\"a\" /* note */ \"b\")",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "ref",
                    "__ref(\"a\" /* note */ \"b\")",
                    14,
                    "__ref(\"a\" /* note */ \"b\") is not a valid __ref() call",
                    "__ref() takes exactly one double-quoted name, with no comments or extra spaces \
                     inside the parentheses: __ref(\"model_name\")",
                    "__ref(\"model_name\")",
                )],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "comments, strings and dollar quotes hide references",
            sql: "-- __ref(a)\nSELECT '__ref(b)', $$__ref(c)$$, /* __ref(d) */ 1 FROM __ref(\"e\")",
            syntax: generic(),
            expected_extraction: extracted(vec![reference("ref", "e")]),
        },
        ExtractSqlReferencesTestCase {
            description: "dialect hash comments hide references",
            sql: "SELECT 1 # __ref(a)\nFROM __ref(\"b\")",
            syntax: backslash_hash_comments(),
            expected_extraction: extracted(vec![reference("ref", "b")]),
        },
        ExtractSqlReferencesTestCase {
            description: "dialect backslash escapes keep strings open",
            sql: "SELECT 'it\\'s __ref(a)' FROM __ref(\"b\")",
            syntax: backslash_hash_comments(),
            expected_extraction: extracted(vec![reference("ref", "b")]),
        },
        ExtractSqlReferencesTestCase {
            description: "non-ascii quoted names are exact",
            sql: "SELECT * FROM __ref(\"commandes_é\")",
            syntax: generic(),
            expected_extraction: extracted(vec![reference("ref", "commandes_é")]),
        },
        ExtractSqlReferencesTestCase {
            description: "empty table function call argument",
            sql: "SELECT * FROM __table_fn(\"f\")(1,,2)",
            syntax: generic(),
            expected_extraction: failed("SQL reference contains an empty argument", 14),
        },
        ExtractSqlReferencesTestCase {
            description: "unclosed table function call",
            sql: "SELECT * FROM __table_fn(\"f\")(1",
            syntax: generic(),
            expected_extraction: failed(
                "SQL table function call contains an unclosed parenthesis",
                14,
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "unclosed reference",
            sql: "SELECT * FROM __ref(orders",
            syntax: generic(),
            expected_extraction: failed("SQL reference contains an unclosed parenthesis", 14),
        },
        ExtractSqlReferencesTestCase {
            description: "an unclosed quote inside a call points at the call",
            sql: "SELECT 'é', __ref(\"a\" || 'open)",
            syntax: generic(),
            expected_extraction: failed("SQL reference contains an unclosed quoted string", 12),
        },
        ExtractSqlReferencesTestCase {
            description: "an unclosed block comment after references points at the comment",
            sql: "SELECT __ref(\"a\") /* é */ /* open",
            syntax: generic(),
            expected_extraction: failed("SQL reference contains an unclosed block comment", 26),
        },
        ExtractSqlReferencesTestCase {
            description: "unclosed quoted text",
            sql: "SELECT 'orders",
            syntax: generic(),
            expected_extraction: failed("SQL reference contains an unclosed quoted string", 7),
        },
        ExtractSqlReferencesTestCase {
            description: "an unquoted name is rejected",
            sql: "SELECT * FROM __ref(orders)",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "ref",
                    "__ref(orders)",
                    14,
                    "__ref(orders) is not a valid __ref() call",
                    "__ref() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __ref(\"orders\")",
                    "__ref(\"orders\")",
                )],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "a single quoted name is rejected",
            sql: "SELECT * FROM __source('raw_orders')",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "source",
                    "__source('raw_orders')",
                    14,
                    "__source('raw_orders') is not a valid __source() call",
                    "__source() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __source(\"raw_orders\")",
                    "__source(\"raw_orders\")",
                )],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "spaces inside the call are rejected",
            sql: "SELECT * FROM __seed( \"regions\" )",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "seed",
                    "__seed( \"regions\" )",
                    14,
                    "__seed( \"regions\" ) is not a valid __seed() call",
                    "__seed() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __seed(\"regions\")",
                    "__seed(\"regions\")",
                )],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "an empty quoted name is rejected",
            sql: "SELECT * FROM __ref(\"\")",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "ref",
                    "__ref(\"\")",
                    14,
                    "__ref(\"\") is not a valid __ref() call",
                    "__ref() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __ref(\"model_name\")",
                    "__ref(\"model_name\")",
                )],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "a doubled quote inside a name is rejected",
            sql: "SELECT * FROM __ref(\"ord\"\"ers\")",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "ref",
                    "__ref(\"ord\"\"ers\")",
                    14,
                    "__ref(\"ord\"\"ers\") is not a valid __ref() call",
                    "__ref() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __ref(\"model_name\")",
                    "__ref(\"model_name\")",
                )],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "a second name argument is rejected",
            sql: "SELECT * FROM __ref(\"a\", \"b\")",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "ref",
                    "__ref(\"a\", \"b\")",
                    14,
                    "__ref(\"a\", \"b\") is not a valid __ref() call",
                    "__ref() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __ref(\"model_name\")",
                    "__ref(\"model_name\")",
                )],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "a dbt reference trailing space is rejected",
            sql: "SELECT * FROM __dbt_ref(\"shop\", \"customers\" )",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "dbt_ref",
                    "__dbt_ref(\"shop\", \"customers\" )",
                    14,
                    "__dbt_ref(\"shop\", \"customers\" ) is not a valid __dbt_ref() call",
                    "__dbt_ref() takes one double-quoted model name, or a double-quoted package name and model name separated by a comma, with no comments or extra spaces inside the parentheses: __dbt_ref(\"shop\", \"customers\")",
                    "__dbt_ref(\"shop\", \"customers\")",
                )],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "a dbt reference third name is rejected",
            sql: "SELECT * FROM __dbt_ref(\"a\", \"b\", \"c\")",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "dbt_ref",
                    "__dbt_ref(\"a\", \"b\", \"c\")",
                    14,
                    "__dbt_ref(\"a\", \"b\", \"c\") is not a valid __dbt_ref() call",
                    "__dbt_ref() takes one double-quoted model name, or a double-quoted package name and model name separated by a comma, with no comments or extra spaces inside the parentheses: __dbt_ref(\"package_name\", \"model_name\")",
                    "__dbt_ref(\"package_name\", \"model_name\")",
                )],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "non-ascii space around a dbt comma is python whitespace",
            sql: "SELECT * FROM __dbt_ref(\"shop\"\u{a0}, \"customers\")",
            syntax: generic(),
            expected_extraction: rejected(vec![dbt_reference("shop", "customers")], vec![]),
        },
        ExtractSqlReferencesTestCase {
            description: "a table function without a call suffix is rejected",
            sql: "SELECT * FROM __table_fn(\"f\") /* x */ (1)",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "table_fn",
                    "__table_fn(\"f\")",
                    14,
                    "__table_fn must be followed by an argument list",
                    "pass the function arguments in a second set of parentheses, using () for no arguments: __table_fn(\"f\")()",
                    "__table_fn(\"f\")()",
                )],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "a single quoted table function name is rejected",
            sql: "SELECT * FROM __table_fn('f')(1)",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "table_fn",
                    "__table_fn('f')",
                    14,
                    "__table_fn('f') is not a valid __table_fn() call",
                    "__table_fn() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __table_fn(\"f\")(...)",
                    "__table_fn(\"f\")(...)",
                )],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "a nested reference name is rejected",
            sql: "SELECT * FROM __ref(__ref(\"a\"))",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "ref",
                    "__ref(__ref(\"a\"))",
                    14,
                    "__ref(__ref(\"a\")) is not a valid __ref() call",
                    "__ref() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __ref(\"model_name\")",
                    "__ref(\"model_name\")",
                )],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "a non-ascii identifier is rejected at its code-point offset",
            sql: "SELECT '\u{e9}', __ref(command\u{e9}s)",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "ref",
                    "__ref(command\u{e9}s)",
                    12,
                    "__ref(command\u{e9}s) is not a valid __ref() call",
                    "__ref() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __ref(\"model_name\")",
                    "__ref(\"model_name\")",
                )],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "rejected calls and references keep authored order",
            sql: "SELECT * FROM __ref(\"a\") JOIN __ref(b) JOIN __udf(c, d)(1)",
            syntax: generic(),
            expected_extraction: rejected(
                vec![reference("ref", "a")],
                vec![
                    invalid_call(
                        "ref",
                        "__ref(b)",
                        30,
                        "__ref(b) is not a valid __ref() call",
                        "__ref() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __ref(\"b\")",
                        "__ref(\"b\")",
                    ),
                    invalid_call(
                        "udf",
                        "__udf(c, d)",
                        44,
                        "__udf(c, d) is not a valid __udf() call",
                        "__udf() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __udf(\"function_name\")",
                        "__udf(\"function_name\")",
                    ),
                ],
            ),
        },
        ExtractSqlReferencesTestCase {
            description: "non-ascii whitespace before a call suffix is python whitespace",
            sql: "SELECT * FROM __table_fn(\"f\")\u{a0}(1)",
            syntax: generic(),
            expected_extraction: rejected(vec![table_function("f", 1)], vec![]),
        },
        ExtractSqlReferencesTestCase {
            description: "python whitespace includes ascii separators",
            sql: "SELECT * FROM __table_fn(\"f\")\u{3000}(1)",
            syntax: generic(),
            expected_extraction: extracted(vec![table_function("f", 1)]),
        },
        ExtractSqlReferencesTestCase {
            description: "non-ASCII text inside and around a rejected call keeps its characters",
            sql: "SELECT 'é' FROM __ref(größe, \u{3000}\"b\")",
            syntax: generic(),
            expected_extraction: rejected(
                vec![],
                vec![invalid_call(
                    "ref",
                    "__ref(größe, \u{3000}\"b\")",
                    16,
                    "__ref(größe, \"b\") is not a valid __ref() call",
                    "__ref() takes exactly one double-quoted name, with no comments or extra spaces inside the parentheses: __ref(\"model_name\")",
                    "__ref(\"model_name\")",
                )],
            ),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            extract_sql_references(test_case.sql, &test_case.syntax),
            test_case.expected_extraction,
            "{}",
            test_case.description
        );
    }
}
