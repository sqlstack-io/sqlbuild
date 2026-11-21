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
            expected: Ok(
                "SELECT s.a FROM p RIGHT JOIN (SELECT 1 AS __sqb_partition_inputs) AS \
                 __sqb_partition_inputs_0 ON FALSE CROSS JOIN __table_fn(\"f\")(p.x) OVER \
                 (PARTITION BY p.k) s",
            ),
        },
        PartitionInputsTestCase {
            description: "earlier comma inputs become cross joins so the collapse covers them all",
            sql: "SELECT s.a FROM a, (SELECT 1, 2) b, LATERAL __table_fn(\"f\")(b.x) over (partition by b.k) AS s, c",
            expected: Ok(
                "SELECT s.a FROM a CROSS JOIN (SELECT 1, 2) b RIGHT JOIN (SELECT 1 AS \
                 __sqb_partition_inputs) AS __sqb_partition_inputs_0 ON FALSE CROSS JOIN LATERAL \
                 __table_fn(\"f\")(b.x) over (partition by b.k) AS s, c",
            ),
        },
        PartitionInputsTestCase {
            description: "a join before the call keeps its condition",
            sql: "SELECT s.a FROM p LEFT JOIN __table_fn(\"f\")(p.x) OVER (PARTITION BY p.k) s ON TRUE",
            expected: Ok(
                "SELECT s.a FROM p RIGHT JOIN (SELECT 1 AS __sqb_partition_inputs) AS \
                 __sqb_partition_inputs_0 ON FALSE LEFT JOIN __table_fn(\"f\")(p.x) OVER \
                 (PARTITION BY p.k) s ON TRUE",
            ),
        },
        PartitionInputsTestCase {
            description: "a call inside a subquery stops at its own FROM, past comments and strings",
            sql: "SELECT * FROM t, (SELECT s.a FROM p /* , */, __table_fn(\"f\")(',', p.x) OVER (PARTITION BY p.k) s) q",
            expected: Ok(
                "SELECT * FROM t, (SELECT s.a FROM p /* , */ RIGHT JOIN (SELECT 1 AS \
                 __sqb_partition_inputs) AS __sqb_partition_inputs_0 ON FALSE CROSS JOIN \
                 __table_fn(\"f\")(',', p.x) OVER (PARTITION BY p.k) s) q",
            ),
        },
        PartitionInputsTestCase {
            description: "a call right after FROM has no inputs to collapse",
            sql: "SELECT * FROM __table_fn(\"f\")(1) OVER (PARTITION BY 1) s",
            expected: Ok("SELECT * FROM __table_fn(\"f\")(1) OVER (PARTITION BY 1) s"),
        },
        PartitionInputsTestCase {
            description: "an unpartitioned call stays joined to each input row",
            sql: "SELECT s.a FROM p, __table_fn(\"f\")(p.x) s",
            expected: Ok("SELECT s.a FROM p, __table_fn(\"f\")(p.x) s"),
        },
        PartitionInputsTestCase {
            description: "a call without a fixture is left alone",
            sql: "SELECT s.a FROM p, __table_fn(\"g\")(p.x) OVER (PARTITION BY p.k) s",
            expected: Ok("SELECT s.a FROM p, __table_fn(\"g\")(p.x) OVER (PARTITION BY p.k) s"),
        },
        PartitionInputsTestCase {
            description: "a partitioned call outside a FROM clause is an error",
            sql: "SELECT (__table_fn(\"f\")(1) OVER (PARTITION BY 1))",
            expected: Err(
                "a __table_fn__f fixture replaces a call with an OVER clause, which must stand in \
                 a FROM clause",
            ),
        },
    ];
    let pattern = Regex::new(r#"(?i)__table_fn\("([^"]+)"\)"#).expect("valid marker pattern");
    let syntax = LexicalSyntax {
        line_comment_prefixes: vec!["--".to_string()],
        ..LexicalSyntax::default()
    };
    for test_case in test_cases {
        let collapsed =
            collapse_partition_inputs(test_case.sql, &pattern, &syntax, |name| name == "f");
        match test_case.expected {
            Ok(expected) => assert_eq!(
                collapsed.as_deref(),
                Ok(expected),
                "{}",
                test_case.description
            ),
            Err(fragment) => assert!(
                collapsed
                    .as_ref()
                    .is_err_and(|error| error.contains(fragment)),
                "{}: {collapsed:?}",
                test_case.description
            ),
        }
    }
}
