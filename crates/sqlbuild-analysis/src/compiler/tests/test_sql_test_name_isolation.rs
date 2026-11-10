use std::collections::HashMap;

use crate::compiler::_helpers::sql_tests::helper_names::rename_helper_references;
use crate::compiler::_helpers::sql_tests::rendering::sql_test_parser;
use crate::compiler::tests::helpers::{helper_names_are_isolated, plan_shape_refusal};
use crate::compiler::tests::test_types::{
    HelperRenameTestCase, NameRefusalTestCase, PlanShape, SharedNameTestCase,
};

const SHARED_CTE_NAME_MODEL: &str = "WITH base_rows AS (SELECT order_id FROM __source(\"raw_orders\")) \
     SELECT base_rows.order_id FROM base_rows";
const PHYSICAL_TABLE_MODEL: &str = "WITH final AS (SELECT o.order_id FROM __source(\"raw_orders\") AS o \
     INNER JOIN lookup_rows AS l ON l.order_id = o.order_id) SELECT order_id FROM final";

#[test]
fn given_helper_references_when_renaming_then_only_relation_references_change() {
    let test_cases = [
        HelperRenameTestCase {
            description: "unaliased reference keeps its spelling as the alias",
            sql: "SELECT expected_rows.order_id FROM expected_rows",
            expected_sql: Some(
                "SELECT expected_rows.order_id FROM __helper__expected_rows AS expected_rows",
            ),
        },
        HelperRenameTestCase {
            description: "aliased references keep their alias",
            sql: "SELECT e.order_id FROM expected_rows AS e JOIN Expected_Rows f ON e.order_id = f.order_id",
            expected_sql: Some(
                "SELECT e.order_id FROM __helper__expected_rows AS e JOIN __helper__expected_rows f ON e.order_id = f.order_id",
            ),
        },
        HelperRenameTestCase {
            description: "columns, aliases, strings and qualified tables named like a helper stay",
            sql: "SELECT expected_rows AS expected_rows, 'expected_rows' AS label \
                  FROM main.expected_rows -- expected_rows",
            expected_sql: Some(
                "SELECT expected_rows AS expected_rows, 'expected_rows' AS label \
                 FROM main.expected_rows -- expected_rows",
            ),
        },
        HelperRenameTestCase {
            description: "references in subqueries, join conditions and set operations change",
            sql: "SELECT '订单' AS label FROM __ref(\"orders\") AS o \
                  JOIN t ON o.order_id IN (SELECT order_id FROM expected_rows) \
                  EXCEPT SELECT '订单', order_id FROM \"expected_rows\"",
            expected_sql: Some(
                "SELECT '订单' AS label FROM __ref(\"orders\") AS o \
                 JOIN t ON o.order_id IN (SELECT order_id FROM __helper__expected_rows AS expected_rows) \
                 EXCEPT SELECT '订单', order_id FROM __helper__expected_rows AS \"expected_rows\"",
            ),
        },
        HelperRenameTestCase {
            description: "SQL that does not parse is not rewritten",
            sql: "SELECT FROM WHERE expected_rows",
            expected_sql: None,
        },
    ];
    let helpers: HashMap<String, String> = HashMap::from([(
        "expected_rows".to_string(),
        "__helper__expected_rows".to_string(),
    )]);
    let dialect = sql_test_parser(Some("duckdb")).expect("DuckDB parser");
    for test_case in test_cases {
        assert_eq!(
            rename_helper_references(test_case.sql, &helpers, dialect).as_deref(),
            test_case.expected_sql,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_helpers_sharing_model_names_when_rendering_then_names_are_isolated_on_every_dialect() {
    let test_cases = [
        SharedNameTestCase {
            description: "helper read by a fixture shares a model's CTE name",
            model_sql: SHARED_CTE_NAME_MODEL,
            helper_name: "base_rows",
            expected_model_reads: "SELECT base_rows.order_id FROM base_rows",
            expected_isolated: true,
        },
        SharedNameTestCase {
            description: "helper read by a fixture shares a physical table the model reads",
            model_sql: PHYSICAL_TABLE_MODEL,
            helper_name: "lookup_rows",
            expected_model_reads: "INNER JOIN lookup_rows AS l",
            expected_isolated: true,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            helper_names_are_isolated(&test_case),
            test_case.expected_isolated,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_names_that_cannot_be_isolated_when_planning_then_test_is_refused() {
    let test_cases = [
        NameRefusalTestCase {
            description: "model CTE uses a generated helper name",
            shape: PlanShape {
                dialect: "snowflake",
                sql_analysis_enabled: true,
                model_sql: "WITH staged AS (WITH __helper__base_rows AS (SELECT 2 AS order_id) \
                            SELECT order_id FROM __source(\"raw_orders\")) SELECT order_id FROM staged",
                helper_name: "base_rows",
                expected_sql: "SELECT 1 AS order_id",
            },
            expected_message: "compile_input:SQL test 'tests/orders.sql' reads model 'orders', \
                 which defines CTE '__helper__base_rows'; names starting with a SQLBuild test \
                 prefix such as __helper__ or __ref__ are reserved for the test query, so rename \
                 the CTE in the model",
        },
        NameRefusalTestCase {
            description: "test CTE redefines a helper in its own WITH",
            shape: PlanShape {
                dialect: "duckdb",
                sql_analysis_enabled: true,
                model_sql: SHARED_CTE_NAME_MODEL,
                helper_name: "base_rows",
                expected_sql: "WITH base_rows AS (SELECT 1 AS order_id) SELECT order_id FROM base_rows",
            },
            expected_message: "compile_input:SQL test 'tests/orders.sql' defines CTE 'base_rows' \
                 inside CTE '__expected__orders', which redefines helper CTE 'base_rows'; give one \
                 of them a different name",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            plan_shape_refusal(&test_case.shape),
            test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}
