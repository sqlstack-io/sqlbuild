//! Logical cursor bounds share one physical read per relation column and type.

use crate::planner_warehouse::main::cursor_batch_sql::cursor_batch_sql;
use crate::planner_warehouse::main::cursor_query_groups::cursor_query_groups;
use crate::planner_warehouse::main::cursor_query_identity::cursor_query_identity;
use crate::planner_warehouse::main::cursor_query_sql::cursor_query_sql;
use crate::planner_warehouse::models::PhysicalCursorQuery;
use crate::planner_warehouse::tests::helpers::request;
use crate::planner_warehouse::tests::test_types::CursorGroupTestCase;

#[test]
fn given_logical_bounds_when_grouping_then_reads_each_relation_once_in_first_order() {
    let test_cases = [
        CursorGroupTestCase {
            description: "target max and shared upstream bounds merge",
            requests: vec![
                request("analytics.orders", None, "orders__target__max"),
                request(
                    "raw.events",
                    Some("orders__events__min"),
                    "orders__events__max",
                ),
                request(
                    "raw.events",
                    Some("totals__events__min"),
                    "totals__events__max",
                ),
                request(
                    "raw.events",
                    Some("orders__events__min"),
                    "orders__events__max",
                ),
            ],
            expected_sql: vec![
                "SELECT CAST(MAX(updated_at) AS VARCHAR) AS _max FROM analytics.orders",
                "SELECT CAST(MIN(updated_at) AS VARCHAR) AS _min, CAST(MAX(updated_at) AS VARCHAR) AS _max FROM raw.events",
            ],
            expected_identities: vec![
                "(1/2): analytics.orders.updated_at [max]",
                "(2/2): raw.events.updated_at [min,max]",
            ],
            expected_batch_sql: "SELECT 0 AS _read, CAST(MAX(updated_at) AS VARCHAR) AS _min, CAST(MAX(updated_at) AS VARCHAR) AS _max FROM analytics.orders\nUNION ALL\nSELECT 1 AS _read, CAST(MIN(updated_at) AS VARCHAR) AS _min, CAST(MAX(updated_at) AS VARCHAR) AS _max FROM raw.events",
        },
        CursorGroupTestCase {
            description: "no requests read nothing",
            requests: Vec::new(),
            expected_sql: Vec::new(),
            expected_identities: Vec::new(),
            expected_batch_sql: "",
        },
    ];
    for test_case in test_cases {
        let groups = cursor_query_groups(&test_case.requests);
        let batch: Vec<&PhysicalCursorQuery> = groups.iter().collect();
        let batch_sql: String = cursor_batch_sql(&batch, &|query, function| {
            format!("CAST({function}({}) AS VARCHAR)", query.cursor_column)
        });
        assert_eq!(
            batch_sql, test_case.expected_batch_sql,
            "{}",
            test_case.description
        );
        let sql: Vec<String> = groups
            .iter()
            .map(|group| {
                cursor_query_sql(group, &|function| {
                    format!("CAST({function}({}) AS VARCHAR)", group.cursor_column)
                })
            })
            .collect();
        let identities: Vec<String> = groups
            .iter()
            .enumerate()
            .map(|(index, group)| cursor_query_identity(group, (index + 1, groups.len())))
            .collect();
        assert_eq!(sql, test_case.expected_sql, "{}", test_case.description);
        assert_eq!(
            identities, test_case.expected_identities,
            "{}",
            test_case.description
        );
    }
}
