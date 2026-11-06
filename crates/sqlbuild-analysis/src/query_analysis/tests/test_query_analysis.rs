use crate::query_analysis::engine::{analyze_json, analyze_project_json};
use crate::query_analysis::tests::helpers::{
    binding_warnings_preserve_declared_relationship_evidence,
    borrowed_facts_preserve_named_outputs_and_terminal_sources,
    borrowed_facts_preserve_union_dependencies_and_join_nullability,
    bound_lineage_survives_type_recovery, canonical_queries_reuse_semantics_and_project_resources,
    catalog_views_reuse_one_analysis_pool, combined_queries_preserve_standalone_binding,
    compact_project_query_interns_repeated_lineage_strings,
    concurrent_batches_match_serial_batches, interleaved_query_templates_preserve_template_order,
    larger_later_queries_keep_request_order, native_compatibility_types_preserve_result_semantics,
    repeated_project_facts_intern_complete_facts,
    unannotated_types_require_complete_nested_evidence,
    widening_aggregates_require_compatibility_recovery,
};
use crate::query_analysis::tests::test_types::{
    CompactQueryAnalysisTestCase, QueryAnalysisExpectedValue, QueryAnalysisTestCase,
};
use serde_json::{Value, json};

#[test]
fn given_compact_query_cases_when_analyzing_projects_then_expected_behavior_holds() {
    let test_cases = [
        CompactQueryAnalysisTestCase {
            description: "binding warnings preserve declared relationship evidence",
            run: binding_warnings_preserve_declared_relationship_evidence,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "unannotated types require complete nested evidence",
            run: unannotated_types_require_complete_nested_evidence,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "borrowed facts preserve named outputs and actual terminal columns",
            run: borrowed_facts_preserve_named_outputs_and_terminal_sources,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "borrowed output graph preserves union dependencies and outer-join nullability",
            run: borrowed_facts_preserve_union_dependencies_and_join_nullability,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "borrowed native type evaluation preserves expression result semantics",
            run: native_compatibility_types_preserve_result_semantics,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "combined compilation preserves standalone analysis and binding",
            run: combined_queries_preserve_standalone_binding,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "widening aggregates preserve compatibility recovery",
            run: widening_aggregates_require_compatibility_recovery,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "resolved bound lineage survives type recovery",
            run: bound_lineage_survives_type_recovery,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "compact project queries intern repeated lineage strings",
            run: compact_project_query_interns_repeated_lineage_strings,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "repeated project facts share compact fact rows",
            run: repeated_project_facts_intern_complete_facts,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "canonical queries reuse semantics while projecting resource names",
            run: canonical_queries_reuse_semantics_and_project_resources,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "interleaved query templates preserve request order",
            run: interleaved_query_templates_preserve_template_order,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "larger later queries keep request order",
            run: larger_later_queries_keep_request_order,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "catalog views reuse one analysis pool",
            run: catalog_views_reuse_one_analysis_pool,
            expected_success: true,
        },
        CompactQueryAnalysisTestCase {
            description: "concurrent batches match serial batches",
            run: concurrent_batches_match_serial_batches,
            expected_success: true,
        },
    ];

    for test_case in test_cases {
        let actual_success = (test_case.run)();
        assert_eq!(
            actual_success, test_case.expected_success,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_query_analysis_cases_when_analyzing_batch_then_returns_expected_facts() {
    let test_cases = [
        QueryAnalysisTestCase {
            description: "cast output retains both named union branches",
            request: json!({"requests": [{
                "sql": "WITH combined AS (SELECT order_id FROM orders UNION ALL BY NAME SELECT customer_id AS order_id FROM customers), typed AS (SELECT CAST(order_id AS BIGINT) AS order_id FROM combined) SELECT order_id FROM typed",
                "dialect": "snowflake", "recoverCteFacts": true,
                "references": {
                    "orders": {"resourceType": "model", "resourceName": "orders"},
                    "customers": {"resourceType": "model", "resourceName": "customers"}
                }
            }]}),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/lineageColumns/0/upstreamColumns/0/resourceName",
                    value: "customers",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/lineageColumns/0/upstreamColumns/1/resourceName",
                    value: "orders",
                },
            ],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "CTE comparisons preserve Boolean result types",
            request: json!({"requests": [{
                "sql": "WITH selected AS (SELECT CAST(1 AS BIGINT) AS order_id) SELECT order_id > 0 AS result FROM selected",
                "dialect": "duckdb", "recoverCteFacts": true, "richTypeInference": false
            }]}),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![QueryAnalysisExpectedValue {
                pointer: "/0/analysis/columns/0/type",
                value: "BOOLEAN",
            }],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "raw batches preserve order and isolate parse errors",
            request: json!({
                "workers": 2,
                "requests": [
                    {"sql": "SELECT order_id FROM orders", "dialect": "duckdb"},
                    {"sql": "SELECT FROM", "dialect": "duckdb"},
                    {"sql": "SELECT product_id FROM products", "dialect": "duckdb"}
                ]
            }),
            analyze: analyze_json,
            expected_length: 3,
            expected_values: vec![
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/projections/0/name",
                    value: "order_id",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/2/analysis/projections/0/name",
                    value: "product_id",
                },
            ],
            expected_nonempty_strings: vec!["/1/error"],
        },
        QueryAnalysisTestCase {
            description: "schema analysis returns typed nullable projections",
            request: json!({
                "requests": [{
                    "sql": "SELECT order_id FROM orders",
                    "dialect": "duckdb",
                    "schema": {"tables": [{
                        "name": "orders",
                        "columns": [{"name": "order_id", "type": "BIGINT", "nullable": false}]
                    }]}
                }]
            }),
            analyze: analyze_json,
            expected_length: 1,
            expected_values: vec![
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/projections/0/typeHint",
                    value: "BIGINT",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/projections/0/nullability",
                    value: "non_null",
                },
            ],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "project analysis returns compact lineage facts",
            request: json!({
                "requests": [{
                    "sql": "SELECT order_id FROM orders",
                    "dialect": "duckdb",
                    "schema": {"tables": [{
                        "name": "orders",
                        "columns": [{"name": "order_id", "type": "BIGINT", "nullable": false}]
                    }]},
                    "references": {
                        "orders": {"resourceType": "model", "resourceName": "orders"}
                    },
                    "declaredColumnOrder": ["order_id"]
                }]
            }),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/columns/0/name",
                    value: "order_id",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/lineageColumns/0/upstreamColumns/0/resourceType",
                    value: "model",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/lineageColumns/0/upstreamColumns/0/resourceName",
                    value: "orders",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/lineageColumns/0/upstreamColumns/0/columnName",
                    value: "order_id",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/lineageColumns/0/confidence",
                    value: "high",
                },
            ],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "CTE recovery projects passthrough types",
            request: json!({
                "requests": [{
                    "sql": "WITH selected AS (SELECT CAST(order_id AS BIGINT) AS order_id FROM orders) SELECT order_id FROM selected",
                    "dialect": "duckdb",
                    "recoverCteFacts": true
                }]
            }),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/columns/0/name",
                    value: "order_id",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/columns/0/type",
                    value: "BIGINT",
                },
            ],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "root schema type takes precedence over normalized CTE recovery",
            request: json!({
                "requests": [{
                    "sql": "WITH selected AS (SELECT attributes FROM orders UNION ALL SELECT attributes FROM orders) SELECT attributes FROM selected",
                    "dialect": "snowflake",
                    "schema": {"tables": [{
                        "name": "orders",
                        "columns": [{"name": "attributes", "type": "VARIANT"}]
                    }]},
                    "richTypeInference": false,
                    "recoverCteFacts": true
                }]
            }),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![QueryAnalysisExpectedValue {
                pointer: "/0/analysis/columns/0/type",
                value: "VARIANT",
            }],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "specialized aggregate return types propagate through CTEs",
            request: json!({
                "requests": [{
                    "sql": "WITH aggregated AS (SELECT OBJECT_AGG(product_id, TO_VARIANT(quantity)) AS inventory FROM products), selected AS (SELECT inventory FROM aggregated) SELECT inventory FROM selected",
                    "dialect": "snowflake",
                    "schema": {"tables": [{
                        "name": "products",
                        "columns": [
                            {"name": "product_id", "type": "NUMBER"},
                            {"name": "quantity", "type": "NUMBER"}
                        ]
                    }]},
                    "functionReturnTypes": {"OBJECT_AGG": "OBJECT", "TO_VARIANT": "VARIANT"},
                    "recoverCteFacts": true
                }]
            }),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![QueryAnalysisExpectedValue {
                pointer: "/0/analysis/columns/0/type",
                value: "OBJECT",
            }],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "outer casts take precedence over nested function return types",
            request: json!({
                "requests": [{
                    "sql": "WITH converted AS (SELECT CAST(TO_OBJECT(attributes) AS VARIANT) AS attributes FROM products) SELECT attributes FROM converted",
                    "dialect": "snowflake",
                    "schema": {"tables": [{
                        "name": "products",
                        "columns": [{"name": "attributes", "type": "TEXT"}]
                    }]},
                    "functionReturnTypes": {"TO_OBJECT": "OBJECT"},
                    "recoverCteFacts": true
                }]
            }),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![QueryAnalysisExpectedValue {
                pointer: "/0/analysis/columns/0/type",
                value: "VARIANT",
            }],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "type-preserving aggregates inherit CTE argument types",
            request: json!({
                "requests": [{
                    "sql": "WITH typed AS (SELECT CAST(created_at AS TIMESTAMP_NTZ) AS created_at, CAST(is_active AS BOOLEAN) AS is_active FROM orders), aggregated AS (SELECT MAX(IFF(is_active, created_at, NULL)) AS latest_at FROM typed) SELECT latest_at FROM aggregated",
                    "dialect": "snowflake",
                    "recoverCteFacts": true
                }]
            }),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![QueryAnalysisExpectedValue {
                pointer: "/0/analysis/columns/0/type",
                value: "TIMESTAMPNTZ",
            }],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "conditional expressions inherit CTE value types",
            request: json!({
                "requests": [{
                    "sql": "WITH typed AS (SELECT CAST(fulfilled_at AS DATE) AS fulfilled_at FROM orders), selected AS (SELECT COALESCE(fulfilled_at, CAST(NULL AS DATE)) AS fulfilled_at FROM typed) SELECT fulfilled_at FROM selected",
                    "dialect": "snowflake",
                    "recoverCteFacts": true,
                    "richTypeInference": false
                }]
            }),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![QueryAnalysisExpectedValue {
                pointer: "/0/analysis/columns/0/type",
                value: "DATE",
            }],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "three-stage set operation CTEs preserve expression types",
            request: json!({
                "requests": [{
                    "sql": "WITH current_orders AS (SELECT CAST(status AS VARCHAR) AS status FROM orders), combined AS (SELECT status FROM current_orders UNION ALL SELECT CAST(NULL AS VARCHAR) AS status FROM orders), final AS (SELECT status FROM combined) SELECT status FROM final",
                    "dialect": "duckdb",
                    "schema": {"tables": [{
                        "name": "orders",
                        "columns": [{"name": "status", "type": "VARCHAR", "nullable": true}]
                    }]},
                    "recoverCteFacts": true
                }]
            }),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![QueryAnalysisExpectedValue {
                pointer: "/0/analysis/columns/0/type",
                value: "TEXT",
            }],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "typed set operation CTEs preserve all contract types",
            request: json!({
                "requests": [{
                    "sql": "WITH current_orders AS (SELECT CAST(category AS VARCHAR) AS category, id IN (1, 2) AS is_selected, TO_TIMESTAMP(0) AS processed_at, CAST(category AS VARCHAR) AS notes FROM upstream), combined AS (SELECT category, is_selected, processed_at, notes FROM current_orders -- Preserve archived records.\nUNION ALL SELECT category, TRUE AS is_selected, CAST(NULL AS TIMESTAMP) AS processed_at, NULL AS notes FROM upstream), final AS (SELECT category, is_selected, processed_at, notes FROM combined) SELECT category, is_selected, processed_at, notes FROM final",
                    "dialect": "snowflake",
                    "schema": {"tables": [{
                        "name": "upstream",
                        "columns": [
                            {"name": "id", "type": "INTEGER", "nullable": false},
                            {"name": "category", "type": "VARCHAR", "nullable": false}
                        ]
                    }]},
                    "functionReturnTypes": {"TO_TIMESTAMP": "TIMESTAMP_NTZ"},
                    "references": {
                        "upstream": {"resourceType": "model", "resourceName": "upstream"}
                    },
                    "recoverCteFacts": true
                }]
            }),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/columns/0/type",
                    value: "TEXT",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/columns/1/type",
                    value: "BOOLEAN",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/columns/2/type",
                    value: "TIMESTAMP_NTZ",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/columns/3/type",
                    value: "TEXT",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/lineageColumns/0/upstreamColumns/0/resourceName",
                    value: "upstream",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/lineageColumns/0/upstreamColumns/0/columnName",
                    value: "category",
                },
            ],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "unbounded string casts preserve legacy type spelling",
            request: json!({
                "requests": [{
                    "sql": "SELECT CAST('new' AS VARCHAR) AS status",
                    "dialect": "duckdb"
                }]
            }),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![QueryAnalysisExpectedValue {
                pointer: "/0/analysis/columns/0/type",
                value: "TEXT",
            }],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "constant casts return compact canonical types",
            request: json!({
                "requests": [{
                    "sql": "SELECT CAST('new' AS VARCHAR(3)) AS status",
                    "dialect": "duckdb"
                }]
            }),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/columns/0/type",
                    value: "VARCHAR(3)",
                },
                QueryAnalysisExpectedValue {
                    pointer: "/0/analysis/lineageColumns/0/transformKind",
                    value: "cast",
                },
            ],
            expected_nonempty_strings: vec![],
        },
        QueryAnalysisTestCase {
            description: "conjunctive non-null filters refine projected nullability",
            request: json!({
                "requests": [{
                    "sql": "SELECT o.order_id FROM orders o WHERE o.order_id IS NOT NULL",
                    "dialect": "duckdb",
                    "schema": {"tables": [{
                        "name": "orders",
                        "columns": [{"name": "order_id", "type": "INT", "nullable": true}]
                    }]},
                    "references": {
                        "orders": {"resourceType": "model", "resourceName": "orders"}
                    }
                }]
            }),
            analyze: analyze_project_json,
            expected_length: 1,
            expected_values: vec![QueryAnalysisExpectedValue {
                pointer: "/0/analysis/columns/0/nullability",
                value: "non_null",
            }],
            expected_nonempty_strings: vec![],
        },
    ];

    for test_case in test_cases {
        let response_json = (test_case.analyze)(&test_case.request.to_string())
            .expect("query analysis request should serialize");
        let response: Value =
            serde_json::from_str(&response_json).expect("query analysis response should be JSON");
        let responses = response
            .as_array()
            .expect("query analysis response should be an array");

        assert_eq!(
            responses.len(),
            test_case.expected_length,
            "{}",
            test_case.description
        );
        for expected_value in test_case.expected_values {
            assert_eq!(
                response
                    .pointer(expected_value.pointer)
                    .and_then(Value::as_str),
                Some(expected_value.value),
                "{}",
                test_case.description
            );
        }
        for expected_pointer in test_case.expected_nonempty_strings {
            assert!(
                response
                    .pointer(expected_pointer)
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.is_empty()),
                "{}",
                test_case.description
            );
        }
    }
}
