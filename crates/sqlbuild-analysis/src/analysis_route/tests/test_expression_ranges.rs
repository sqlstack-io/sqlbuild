use crate::analysis_route::_helpers::analysis::native_sharing::rebase_ranges;
use crate::analysis_route::models::{DuckDbDialect, NativeAnalysis};
use crate::analysis_route::tests::helpers::{analysed, mapped_texts};
use crate::analysis_route::tests::test_types::{ExpressionRangeTestCase, RebaseRangesTestCase};

#[test]
fn given_compiled_sql_when_mapping_expression_ranges_then_they_cover_its_own_text() {
    let test_cases = [
        ExpressionRangeTestCase {
            description: "cleaning rewrites multi-byte comments, references, placeholders and cursors",
            query_sql: "-- café ünïcode\nWITH x AS (SELECT * FROM __ref(\"orders\") WHERE id > @@@floor \
                        AND id < __cursor_end())\nSELECT x.id AS id, UPPER(x.status) AS loud, 'é' AS mark FROM x",
            placeholders: &[("floor", "100000")],
            typed_calls: &[],
            expected_texts: &[
                ("id", Some("x.id AS id")),
                ("loud", Some("UPPER(x.status) AS loud")),
                ("mark", Some("'é' AS mark")),
            ],
        },
        ExpressionRangeTestCase {
            description: "a typed macro call is marked and a cursor bound replaced before analysis",
            query_sql: "SELECT ROUND(amount * 1.2, 2) AS gross, id AS id FROM orders \
                        WHERE updated_at < __cursor_end()",
            placeholders: &[],
            typed_calls: &[("ROUND(amount * 1.2, 2)", "DECIMAL(18, 2)")],
            expected_texts: &[
                ("gross", Some("ROUND(amount * 1.2, 2) AS gross")),
                ("id", Some("id AS id")),
            ],
        },
        ExpressionRangeTestCase {
            description: "an edge beside a removed reference call is dropped, not moved onto the name",
            query_sql: "SELECT __ref(\"orders\").id AS id, UPPER(status) AS loud FROM __ref(\"orders\")",
            placeholders: &[],
            typed_calls: &[],
            expected_texts: &[("id", None), ("loud", Some("UPPER(status) AS loud"))],
        },
        ExpressionRangeTestCase {
            description: "an edge inside a filled placeholder is dropped; one at its start maps to it",
            query_sql: "SELECT @@@cols AS two, @@@floor + 1 AS above, id AS id FROM orders",
            placeholders: &[("cols", "1 AS one, 2"), ("floor", "10")],
            typed_calls: &[],
            expected_texts: &[
                ("one", None),
                ("two", None),
                ("above", Some("@@@floor + 1 AS above")),
                ("id", Some("id AS id")),
            ],
        },
    ];
    for test_case in test_cases {
        let mapped: Vec<(String, Option<String>)> = mapped_texts(
            test_case.query_sql,
            test_case.placeholders,
            test_case.typed_calls,
        );
        let texts: Vec<(&str, Option<&str>)> = mapped
            .iter()
            .map(|(column, text)| (column.as_str(), text.as_deref()))
            .collect();
        assert_eq!(texts, test_case.expected_texts, "{}", test_case.description);
    }
}

#[test]
fn given_a_shared_analysis_when_rebasing_onto_a_member_then_ranges_follow_or_drop() {
    let test_cases = [
        RebaseRangesTestCase {
            description: "a member that only renames a relation keeps every range",
            leader: "WITH x AS (SELECT * FROM orders) SELECT x.id AS id, UPPER(x.status) AS loud FROM x",
            member: "WITH x AS (SELECT * FROM customer_orders) SELECT x.id AS id, UPPER(x.status) AS loud FROM x",
            expected_texts: &[Some("x.id AS id"), Some("UPPER(x.status) AS loud")],
        },
        RebaseRangesTestCase {
            description: "a member that differs outside identifiers drops every range",
            leader: "SELECT x.id AS id FROM orders AS x",
            member: "SELECT x.id AS id FROM orders AS x WHERE 1 = 1",
            expected_texts: &[None],
        },
    ];
    for test_case in test_cases {
        let mut analysis: NativeAnalysis = analysed(&DuckDbDialect, test_case.leader, &[]);
        rebase_ranges(&mut analysis, test_case.leader, test_case.member);
        let texts: Vec<Option<&str>> = analysis
            .lineage
            .iter()
            .map(|row| {
                row.expression_range
                    .map(|(start, end)| &test_case.member[start..end])
            })
            .collect();
        assert_eq!(texts, test_case.expected_texts, "{}", test_case.description);
    }
}
