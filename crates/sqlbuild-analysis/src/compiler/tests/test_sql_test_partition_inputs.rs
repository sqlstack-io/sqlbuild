use regex::Regex;

use crate::compiler::_helpers::sql_tests::partition_inputs::collapse_partition_inputs;
use crate::compiler::tests::test_types::PartitionInputsTestCase;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

#[test]
fn given_a_mocked_partitioned_call_when_collapsing_then_its_inputs_read_as_one_null_row() {
    let test_cases = [
        PartitionInputsTestCase {
            description: "a comma before the call joins the inputs as one row",
            sql: "SELECT s.a FROM p, __table_fn(\"f\")(p.x) OVER (PARTITION BY p.k) s",
            expected_outcome: Ok(
                "SELECT s.a FROM p RIGHT JOIN (SELECT 1 AS __sqb_partition_inputs) AS \
                 __sqb_partition_inputs_0 ON FALSE CROSS JOIN __table_fn(\"f\")(p.x) OVER \
                 (PARTITION BY p.k) s",
            ),
        },
        PartitionInputsTestCase {
            description: "earlier comma inputs become cross joins so the collapse covers them all",
            sql: "SELECT s.a FROM a, (SELECT 1, 2) b, LATERAL __table_fn(\"f\")(b.x) over (partition by b.k) AS s, c",
            expected_outcome: Ok(
                "SELECT s.a FROM a CROSS JOIN (SELECT 1, 2) b RIGHT JOIN (SELECT 1 AS \
                 __sqb_partition_inputs) AS __sqb_partition_inputs_0 ON FALSE CROSS JOIN LATERAL \
                 __table_fn(\"f\")(b.x) over (partition by b.k) AS s, c",
            ),
        },
        PartitionInputsTestCase {
            description: "a join before the call keeps its condition",
            sql: "SELECT s.a FROM p LEFT JOIN __table_fn(\"f\")(p.x) OVER (PARTITION BY p.k) s ON TRUE",
            expected_outcome: Ok(
                "SELECT s.a FROM p RIGHT JOIN (SELECT 1 AS __sqb_partition_inputs) AS \
                 __sqb_partition_inputs_0 ON FALSE LEFT JOIN __table_fn(\"f\")(p.x) OVER \
                 (PARTITION BY p.k) s ON TRUE",
            ),
        },
        PartitionInputsTestCase {
            description: "a call inside a subquery stops at its own FROM, past comments and strings",
            sql: "SELECT * FROM t, (SELECT s.a FROM p /* , */, __table_fn(\"f\")(',', p.x) OVER (PARTITION BY p.k) s) q",
            expected_outcome: Ok(
                "SELECT * FROM t, (SELECT s.a FROM p /* , */ RIGHT JOIN (SELECT 1 AS \
                 __sqb_partition_inputs) AS __sqb_partition_inputs_0 ON FALSE CROSS JOIN \
                 __table_fn(\"f\")(',', p.x) OVER (PARTITION BY p.k) s) q",
            ),
        },
        PartitionInputsTestCase {
            description: "a call right after FROM has no inputs to collapse",
            sql: "SELECT * FROM __table_fn(\"f\")(1) OVER (PARTITION BY 1) s",
            expected_outcome: Ok("SELECT * FROM __table_fn(\"f\")(1) OVER (PARTITION BY 1) s"),
        },
        PartitionInputsTestCase {
            description: "an unpartitioned call stays joined to each input row",
            sql: "SELECT s.a FROM p, __table_fn(\"f\")(p.x) s",
            expected_outcome: Ok("SELECT s.a FROM p, __table_fn(\"f\")(p.x) s"),
        },
        PartitionInputsTestCase {
            description: "a call without a fixture is left alone",
            sql: "SELECT s.a FROM p, __table_fn(\"g\")(p.x) OVER (PARTITION BY p.k) s",
            expected_outcome: Ok(
                "SELECT s.a FROM p, __table_fn(\"g\")(p.x) OVER (PARTITION BY p.k) s",
            ),
        },
        PartitionInputsTestCase {
            description: "a partitioned call in TABLE( ) loses the wrapper and collapses its inputs",
            sql: "SELECT s.a FROM p, TABLE(__table_fn(\"f\")(p.x) OVER (PARTITION BY p.k)) s",
            expected_outcome: Ok(
                "SELECT s.a FROM p RIGHT JOIN (SELECT 1 AS __sqb_partition_inputs) AS \
                 __sqb_partition_inputs_0 ON FALSE CROSS JOIN __table_fn(\"f\")(p.x) OVER \
                 (PARTITION BY p.k) s",
            ),
        },
        PartitionInputsTestCase {
            description: "an unpartitioned call in LATERAL TABLE( ) loses the wrapper only",
            sql: "SELECT s.a FROM p, LATERAL table ( __table_fn(\"f\")(p.x) ) s",
            expected_outcome: Ok("SELECT s.a FROM p, LATERAL __table_fn(\"f\")(p.x) s"),
        },
        PartitionInputsTestCase {
            description: "partition columns the fixture has read the fixture row, outside the call",
            sql: "SELECT p.k, P.K, s.a, p.kk FROM p, __table_fn(\"f\")(p.k) OVER (PARTITION BY p.k) s \
                  WHERE p.k = 1 AND 'p.k' <> q.p.k",
            expected_outcome: Ok(
                "SELECT s.k, s.K, s.a, p.kk FROM p RIGHT JOIN (SELECT 1 AS __sqb_partition_inputs) \
                 AS __sqb_partition_inputs_0 ON FALSE CROSS JOIN __table_fn(\"f\")(p.k) OVER \
                 (PARTITION BY p.k) s WHERE s.k = 1 AND 'p.k' <> q.p.k",
            ),
        },
        PartitionInputsTestCase {
            description: "a call without an alias is named for its partition columns",
            sql: "SELECT p.k FROM p, __table_fn(\"f\")(p.x) OVER (PARTITION BY p.k, p.z ORDER BY p.x)",
            expected_outcome: Ok(
                "SELECT __sqb_partition_call_0.k FROM p RIGHT JOIN (SELECT 1 AS \
                 __sqb_partition_inputs) AS __sqb_partition_inputs_0 ON FALSE CROSS JOIN \
                 __table_fn(\"f\")(p.x) OVER (PARTITION BY p.k, p.z ORDER BY p.x) AS \
                 __sqb_partition_call_0",
            ),
        },
        PartitionInputsTestCase {
            description: "an alias with a column list keeps the partition columns NULL",
            sql: "SELECT p.k FROM p, __table_fn(\"f\")(p.x) OVER (PARTITION BY p.k) s (k, a)",
            expected_outcome: Ok(
                "SELECT p.k FROM p RIGHT JOIN (SELECT 1 AS __sqb_partition_inputs) AS \
                 __sqb_partition_inputs_0 ON FALSE CROSS JOIN __table_fn(\"f\")(p.x) OVER \
                 (PARTITION BY p.k) s (k, a)",
            ),
        },
        PartitionInputsTestCase {
            description: "a join before the call keeps its partition columns, which it reads as inputs",
            sql: "SELECT p.k FROM p JOIN c ON c.id = p.k, __table_fn(\"f\")(p.x) OVER (PARTITION BY p.k) s \
                  JOIN d ON d.id = p.k WHERE p.k = 1",
            expected_outcome: Ok(
                "SELECT s.k FROM p JOIN c ON c.id = p.k RIGHT JOIN (SELECT 1 AS \
                 __sqb_partition_inputs) AS __sqb_partition_inputs_0 ON FALSE CROSS JOIN \
                 __table_fn(\"f\")(p.x) OVER (PARTITION BY p.k) s JOIN d ON d.id = s.k WHERE s.k = 1",
            ),
        },
        PartitionInputsTestCase {
            description: "partition columns read the fixture only in the call's own SELECT",
            sql: "SELECT p.k FROM p UNION ALL SELECT p.k FROM p, __table_fn(\"f\")(p.x) OVER \
                  (PARTITION BY p.k) s",
            expected_outcome: Ok(
                "SELECT p.k FROM p UNION ALL SELECT s.k FROM p RIGHT JOIN (SELECT 1 AS \
                 __sqb_partition_inputs) AS __sqb_partition_inputs_0 ON FALSE CROSS JOIN \
                 __table_fn(\"f\")(p.x) OVER (PARTITION BY p.k) s",
            ),
        },
        PartitionInputsTestCase {
            description: "a partitioned call outside a FROM clause is an error",
            sql: "SELECT (__table_fn(\"f\")(1) OVER (PARTITION BY 1))",
            expected_outcome: Err(
                "compile_input:a __table_fn__f fixture replaces a call with an OVER clause, which \
                 must stand in a FROM clause",
            ),
        },
    ];
    let pattern = Regex::new(r#"(?i)__table_fn\("([^"]+)"\)"#).expect("valid marker pattern");
    let syntax = LexicalSyntax {
        line_comment_prefixes: vec!["--".to_string()],
        ..LexicalSyntax::default()
    };
    for test_case in test_cases {
        let collapsed = collapse_partition_inputs(test_case.sql, &pattern, &syntax, |name| {
            (name == "f").then(|| vec!["k".to_owned(), "a".to_owned()])
        });
        assert_eq!(
            collapsed.as_deref().map_err(String::as_str),
            test_case.expected_outcome,
            "{}",
            test_case.description
        );
    }
}
