use crate::source_drift::main::reads_added_columns::reads_added_columns;
use crate::source_drift::tests::test_types::{AddedColumnsTestCase, DialectAddedColumnsTestCase};

#[test]
fn given_a_source_gaining_columns_when_reading_models_then_only_column_list_readers_are_affected() {
    let test_cases = [
        AddedColumnsTestCase {
            description: "a star over the source",
            sql: r#"SELECT * FROM __source("orders")"#,
            expected_affected: true,
        },
        AddedColumnsTestCase {
            description: "explicit columns from the source alone",
            sql: r#"SELECT order_id, status FROM __source("orders")"#,
            expected_affected: false,
        },
        AddedColumnsTestCase {
            description: "an unqualified name in a join with the source",
            sql: r#"SELECT order_id FROM __source("orders") AS o
                JOIN __ref("customers") AS c ON o.customer_id = c.customer_id"#,
            expected_affected: true,
        },
        AddedColumnsTestCase {
            description: "qualified names in a join with the source",
            sql: r#"SELECT o.order_id, c.customer_name FROM __source("orders") AS o
                JOIN __ref("customers") AS c ON o.customer_id = c.customer_id"#,
            expected_affected: false,
        },
        AddedColumnsTestCase {
            description: "a star reaching the output through a CTE",
            sql: r#"WITH s AS (SELECT * FROM __source("orders")) SELECT * FROM s"#,
            expected_affected: true,
        },
        AddedColumnsTestCase {
            description: "a star through a subquery",
            sql: r#"SELECT * FROM (SELECT * FROM __source("orders")) AS s"#,
            expected_affected: true,
        },
        AddedColumnsTestCase {
            description: "a starred CTE read by explicit columns",
            sql: r#"WITH source_rows AS (SELECT * FROM __source("orders")),
                final AS (SELECT s.order_id, s.status FROM source_rows AS s)
                SELECT * FROM final"#,
            expected_affected: false,
        },
        AddedColumnsTestCase {
            description: "a starred CTE joined with an unqualified name",
            sql: r#"WITH s AS (SELECT * FROM __source("orders"))
                SELECT customer_name FROM s JOIN __ref("customers") AS c ON s.customer_id = c.customer_id"#,
            expected_affected: true,
        },
        AddedColumnsTestCase {
            description: "a set operation with a starred branch",
            sql: r#"SELECT * FROM __source("orders") UNION ALL SELECT * FROM __source("orders")"#,
            expected_affected: true,
        },
        AddedColumnsTestCase {
            description: "a name the source gains, qualified",
            sql: r#"SELECT o.order_id FROM __source("orders") AS o WHERE o.region = 'EU'"#,
            expected_affected: true,
        },
        AddedColumnsTestCase {
            description: "an unqualified name a correlated subquery may now bind to the source",
            sql: r#"SELECT c.customer_id FROM __ref("customers") AS c
                WHERE EXISTS (SELECT 1 FROM __source("orders") AS o WHERE o.customer_id = customer_id)"#,
            expected_affected: true,
        },
        AddedColumnsTestCase {
            description: "a count of every row",
            sql: r#"SELECT COUNT(*) AS orders FROM __source("orders")"#,
            expected_affected: false,
        },
        AddedColumnsTestCase {
            description: "a star over another relation in a join with the source",
            sql: r#"SELECT c.*, o.order_id FROM __ref("customers") AS c
                JOIN __source("orders") AS o ON o.customer_id = c.customer_id"#,
            expected_affected: false,
        },
        AddedColumnsTestCase {
            description: "a star over the source in a join",
            sql: r#"SELECT o.*, c.customer_name FROM __ref("customers") AS c
                JOIN __source("orders") AS o ON o.customer_id = c.customer_id"#,
            expected_affected: true,
        },
        AddedColumnsTestCase {
            description: "SQL the parser cannot read",
            sql: r#"SELECT FROM WHERE __source("orders")"#,
            expected_affected: true,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            reads_added_columns(
                "snowflake",
                (test_case.sql, &[]),
                &["orders".to_owned()],
                &["region".to_owned()],
            ),
            test_case.expected_affected,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_comma_items_with_joins_when_a_joined_source_gains_columns_then_their_keys_and_names_count()
{
    let test_cases = [
        DialectAddedColumnsTestCase {
            description: "postgres: a later comma item joining the source USING a column it gains",
            dialect: "postgres",
            sql: r#"SELECT c.customer_id FROM __ref("customers") AS c,
                __ref("products") AS p JOIN __source("orders") AS o USING (region)"#,
            expected_affected: true,
        },
        DialectAddedColumnsTestCase {
            description: "postgres: an unqualified name in a later comma item's join ON",
            dialect: "postgres",
            sql: r#"SELECT c.customer_id FROM __ref("customers") AS c,
                __ref("products") AS p JOIN __source("orders") AS o ON p.product_id = product_ref"#,
            expected_affected: true,
        },
        DialectAddedColumnsTestCase {
            description: "postgres: qualified names across comma items and their joins",
            dialect: "postgres",
            sql: r#"SELECT c.customer_id FROM __ref("customers") AS c,
                __ref("products") AS p JOIN __source("orders") AS o ON p.product_id = o.product_id"#,
            expected_affected: false,
        },
        DialectAddedColumnsTestCase {
            description: "duckdb: a later comma item joining the source USING a column it gains",
            dialect: "duckdb",
            sql: r#"SELECT c.customer_id FROM __ref("customers") AS c,
                __ref("products") AS p JOIN __source("orders") AS o USING (region)"#,
            expected_affected: true,
        },
        DialectAddedColumnsTestCase {
            description: "duckdb: an unqualified name in a later comma item's join ON",
            dialect: "duckdb",
            sql: r#"SELECT c.customer_id FROM __ref("customers") AS c,
                __ref("products") AS p JOIN __source("orders") AS o ON p.product_id = product_ref"#,
            expected_affected: true,
        },
        DialectAddedColumnsTestCase {
            description: "duckdb: qualified names across comma items and their joins",
            dialect: "duckdb",
            sql: r#"SELECT c.customer_id FROM __ref("customers") AS c,
                __ref("products") AS p JOIN __source("orders") AS o ON p.product_id = o.product_id"#,
            expected_affected: false,
        },
        DialectAddedColumnsTestCase {
            description: "databricks: a later comma item joining the source USING a column it gains",
            dialect: "databricks",
            sql: r#"SELECT c.customer_id FROM __ref("customers") AS c,
                __ref("products") AS p JOIN __source("orders") AS o USING (region)"#,
            expected_affected: true,
        },
        DialectAddedColumnsTestCase {
            description: "databricks: an unqualified name in a later comma item's join ON",
            dialect: "databricks",
            sql: r#"SELECT c.customer_id FROM __ref("customers") AS c,
                __ref("products") AS p JOIN __source("orders") AS o ON p.product_id = product_ref"#,
            expected_affected: true,
        },
        DialectAddedColumnsTestCase {
            description: "databricks: qualified names across comma items and their joins",
            dialect: "databricks",
            sql: r#"SELECT c.customer_id FROM __ref("customers") AS c,
                __ref("products") AS p JOIN __source("orders") AS o ON p.product_id = o.product_id"#,
            expected_affected: false,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            reads_added_columns(
                test_case.dialect,
                (test_case.sql, &[]),
                &["orders".to_owned()],
                &["region".to_owned()],
            ),
            test_case.expected_affected,
            "{}",
            test_case.description
        );
    }
}
