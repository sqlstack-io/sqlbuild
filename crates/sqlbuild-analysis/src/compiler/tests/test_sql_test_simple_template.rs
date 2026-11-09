use std::collections::HashMap;
use std::sync::Mutex;

use crate::compiler::_helpers::sql_tests::planning::{analysis_template, parsed_analysis_template};
use crate::compiler::_helpers::sql_tests::rendering::sql_test_parser;
use crate::compiler::_helpers::sql_tests::simple_template::simple_analysis_template;
use crate::compiler::tests::simple_template_helpers::{
    simple_template_corpus_outcome, template_patterns,
};
use crate::compiler::tests::test_types::{SimpleTemplateCorpusTestCase, SimpleTemplateTestCase};

#[test]
fn given_bounded_and_fallback_queries_when_planning_then_recognized_facts_match_the_parser() {
    let dense = r#"WITH imported AS (SELECT * FROM __ref("orders")),
lookup_0 AS (SELECT * FROM __source("customers")),
projected AS (SELECT CAST(b.id AS INTEGER) AS id,
CAST(b.amount + COALESCE(l0.amount, 0) AS DOUBLE) AS amount,
CAST(COALESCE(b.amount, 0) + CAST(b.id AS DOUBLE) AS DOUBLE) AS quantity
FROM imported AS b LEFT JOIN lookup_0 AS l0 ON l0.id = b.id),
combined AS (SELECT id, amount, quantity FROM projected
UNION ALL BY NAME SELECT id, amount, quantity FROM projected WHERE id = 0),
final AS (SELECT CAST(id AS INTEGER) AS id, CAST(amount AS DOUBLE) AS amount FROM combined)
SELECT id, amount FROM final"#;
    let test_cases = [
        SimpleTemplateTestCase {
            description: "Snowflake dense CTEs, joins and union are recognized",
            dialect: "snowflake",
            sql: dense,
            expected_recognized: true,
        },
        SimpleTemplateTestCase {
            description: "DuckDB dense CTEs, joins and union are recognized",
            dialect: "duckdb",
            sql: dense,
            expected_recognized: true,
        },
        SimpleTemplateTestCase {
            description: "nested CTEs follow outer relation then WITH order",
            dialect: "duckdb",
            sql: "WITH recent AS (WITH nested AS (SELECT * FROM __source(\"orders\")) SELECT id FROM nested) SELECT id FROM recent JOIN __ref(\"customers\") AS c ON c.id = recent.id",
            expected_recognized: true,
        },
        SimpleTemplateTestCase {
            description: "a root union has no CTE names",
            dialect: "duckdb",
            sql: "SELECT id FROM __ref(\"orders\") UNION ALL SELECT id FROM __source(\"orders\")",
            expected_recognized: true,
        },
        SimpleTemplateTestCase {
            description: "two-part dbt references precede joined seeds",
            dialect: "snowflake",
            sql: "SELECT id FROM __dbt_ref(\"shop\", \"orders\") AS o JOIN __seed(\"regions\") AS r ON o.id = r.id",
            expected_recognized: true,
        },
        SimpleTemplateTestCase {
            description: "a line comment leaves the template to the parser",
            dialect: "duckdb",
            sql: "WITH \"recent\" AS (SELECT * FROM __ref(\"or\"\"ders\")) SELECT id FROM \"recent\" -- __seed(\"ignored\")",
            expected_recognized: false,
        },
        SimpleTemplateTestCase {
            description: "a block comment leaves the template to the parser",
            dialect: "duckdb",
            sql: "SELECT id FROM __ref(\"orders\") /* __ref(\"ignored\") */",
            expected_recognized: false,
        },
        SimpleTemplateTestCase {
            description: "Snowflake QUALIFY stays on the parser",
            dialect: "snowflake",
            sql: "SELECT id FROM __ref(\"orders\") QUALIFY ROW_NUMBER() OVER (ORDER BY id) = 1",
            expected_recognized: false,
        },
        SimpleTemplateTestCase {
            description: "a scalar subquery stays on the parser",
            dialect: "duckdb",
            sql: "SELECT (SELECT id FROM __ref(\"orders\")) AS id",
            expected_recognized: false,
        },
        SimpleTemplateTestCase {
            description: "a string marker lookalike stays on the parser",
            dialect: "duckdb",
            sql: "SELECT '__ref(\"ignored\")' AS note FROM __ref(\"orders\")",
            expected_recognized: false,
        },
        SimpleTemplateTestCase {
            description: "a table function call stays on the parser",
            dialect: "duckdb",
            sql: "SELECT id FROM __table_fn(\"orders\")(1)",
            expected_recognized: false,
        },
        SimpleTemplateTestCase {
            description: "WITH over UNION stays on the parser",
            dialect: "duckdb",
            sql: "WITH recent AS (SELECT id FROM orders) SELECT id FROM recent UNION ALL SELECT id FROM recent",
            expected_recognized: false,
        },
        SimpleTemplateTestCase {
            description: "an incomplete cast keeps the parser's refusal",
            dialect: "duckdb",
            sql: "SELECT CAST(id AS INTEGER FROM __ref(\"orders\")",
            expected_recognized: false,
        },
        SimpleTemplateTestCase {
            description: "two statements keep the parser's refusal",
            dialect: "duckdb",
            sql: "SELECT id FROM __ref(\"orders\"); SELECT id FROM other",
            expected_recognized: false,
        },
        SimpleTemplateTestCase {
            description: "non-ASCII text stays on the parser",
            dialect: "duckdb",
            sql: "SELECT \"caf\u{e9}\" FROM __ref(\"orders\")",
            expected_recognized: false,
        },
        SimpleTemplateTestCase {
            description: "Postgres has no recognizer",
            dialect: "postgres",
            sql: "SELECT id FROM __ref(\"orders\")",
            expected_recognized: false,
        },
    ];
    let patterns = template_patterns();
    for test_case in test_cases {
        let parser = sql_test_parser(Some(test_case.dialect)).expect(test_case.description);
        let expected = parsed_analysis_template(test_case.sql, (parser, &patterns));
        let fast = simple_analysis_template(test_case.sql, test_case.dialect, &patterns);
        assert_eq!(
            fast.is_some(),
            test_case.expected_recognized,
            "{}",
            test_case.description
        );
        if fast.is_some() {
            assert_eq!(fast, expected, "{}", test_case.description);
        }
        let cache = Mutex::new(HashMap::new());
        let actual =
            analysis_template(test_case.sql, (parser, &patterns), &cache).expect("template cache");
        assert_eq!(actual, expected, "{}", test_case.description);
    }
}

#[test]
fn given_generated_and_mutated_queries_when_recognized_then_every_fact_matches_the_parser() {
    let test_cases = [
        SimpleTemplateCorpusTestCase {
            description: "DuckDB combinations, token deletions and insertions",
            dialect: "duckdb",
            expected_min_recognized: 200,
            expected_mismatches: &[],
        },
        SimpleTemplateCorpusTestCase {
            description: "Snowflake combinations, token deletions and insertions",
            dialect: "snowflake",
            expected_min_recognized: 200,
            expected_mismatches: &[],
        },
    ];
    for test_case in test_cases {
        let outcome = simple_template_corpus_outcome(test_case.dialect);
        assert!(
            outcome.recognized >= test_case.expected_min_recognized,
            "{}: {} recognized",
            test_case.description,
            outcome.recognized
        );
        assert_eq!(
            outcome.mismatches, test_case.expected_mismatches,
            "{}",
            test_case.description
        );
    }
}
