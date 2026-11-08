use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};

use serde_json::{Value, json};

use crate::compiler::_helpers::sql_tests::rendering::sql_test_parser;
use crate::compiler::_helpers::sql_tests::cte_rename::defined_cte_keys;
use crate::compiler::_helpers::sql_tests::cte_slices::{SliceDialect, split_top_level_with};
use crate::compiler::_helpers::sql_tests::relation_markers::relation_marker_calls;
use crate::compiler::main::sql_test_extraction::extract_batch_json;
use crate::compiler::tests::test_types::ExtractionRuleTestCase;
use crate::compiler::tests::test_types::{
    HelperReferenceTestCase, PlanShape, RenderedShapeTestCase, SharedNameTestCase,
};

pub(crate) fn mixed_expanded_tests_preserve_order_and_payloads() -> bool {
    let response = extracted(r#"{"tests":[{"sql":"WITH helper AS (SELECT 1 AS id), __source__raw_orders AS (SELECT id FROM helper), __expected__orders AS (SELECT id FROM helper), __assert__positive AS (SELECT id FROM helper WHERE id < 0) SELECT 1","fileLabel":"tests/orders.sql","mode":"model"},{"sql":"WITH input AS (SELECT 1 AS value), __udf_actual__ AS (SELECT __udf(\"increment\")(value) AS value FROM input), __udf_expected__ AS (SELECT 2 AS value) SELECT 1","fileLabel":"tests/increment.sql","mode":"udf"}]}"#).expect("batch succeeds");
    let payload: serde_json::Value = serde_json::from_str(&response).expect("valid JSON");
    assert_eq!(payload[0]["kind"], "model");
    assert_eq!(payload[0]["authored"][0][0], "helper");
    assert_eq!(payload[0]["expectedModels"][0], "orders");
    assert_eq!(payload[0]["assertionNames"][0], "positive");
    assert_eq!(payload[1]["kind"], "direct");
    assert_eq!(payload[1]["mode"], "udf");
    assert_eq!(payload[1]["actual"][0], "__udf_actual__");
    true
}

pub(crate) fn trailing_ceremonial_select_is_optional() -> bool {
    let response = extracted(r#"{"tests":[{"sql":"WITH __source__raw_orders AS (SELECT 1 AS id), __expected__orders AS (SELECT 1 AS id); -- done","fileLabel":"tests/orders.sql","mode":"model"}]}"#).expect("omitted select succeeds");
    let payload: serde_json::Value = serde_json::from_str(&response).expect("valid JSON");
    assert_eq!(payload[0]["expectedModels"][0], "orders");

    let error = extracted(r#"{"tests":[{"sql":"WITH __source__raw_orders AS (SELECT 1 AS id), __expected__orders AS (SELECT 1 AS id) SELECT * FROM __expected__orders","fileLabel":"tests/orders.sql","mode":"model"}]}"#).expect_err("other final statement is rejected");
    assert!(error.contains("must end after its CTEs"));
    true
}

pub(crate) fn dependent_assertion_returns_authoritative_error() -> bool {
    let error = extracted(r#"{"tests":[{"sql":"WITH __source__raw_orders AS (SELECT 1 AS id), __expected__orders AS (SELECT 1 AS id), __assert__same AS (SELECT id FROM __expected__orders) SELECT 1","fileLabel":"tests/orders.sql","mode":"model"}]}"#).expect_err("dependency is rejected");
    assert!(error.contains("'__assert__same' must not depend on '__expected__orders'"));
    true
}

pub(crate) fn empty_model_fixture_marker_preserves_direct_mode_validation() -> bool {
    let response = extracted(r#"{"tests":[{"sql":"WITH __source__raw_orders AS (SELECT * FROM __empty_fixture()), __expected__orders AS (SELECT * FROM __empty_fixture()) SELECT 1","fileLabel":"tests/orders.sql","mode":"model"}]}"#).expect("model marker succeeds");
    let payload: serde_json::Value = serde_json::from_str(&response).expect("valid JSON");
    assert_eq!(payload[0]["expectedModels"][0], "orders");

    let error = extracted(r#"{"tests":[{"sql":"WITH __udf_actual__ AS (SELECT 1 AS value), __udf_expected__ AS (SELECT * FROM __empty_fixture()) SELECT 1","fileLabel":"tests/function.sql","mode":"udf"}]}"#).expect_err("direct marker is rejected");
    assert!(error.contains("must not use SELECT * in __udf_expected__ CTEs"));
    true
}

pub(crate) fn expected_projection_errors_name_the_expected_cte() -> bool {
    let cases = [
        (
            r#"{"tests":[{"sql":"WITH __macro_actual__ AS (SELECT @py.status() AS status), __macro_expected__ AS (SELECT 1 + 1) SELECT 1","fileLabel":"tests/status.sql","mode":"macro"}]}"#,
            "SQL test 'tests/status.sql' must alias every non-trivial __macro_expected__ projection",
        ),
        (
            r#"{"tests":[{"sql":"WITH __udf_actual__ AS (SELECT 1 AS value), __udf_expected__ AS (SELECT 1 AS value UNION ALL SELECT 2 AS other) SELECT 1","fileLabel":"tests/function.sql","mode":"udf"}]}"#,
            "SQL test 'tests/function.sql' must use the same __udf_expected__ projection names and order in every set-operation branch; branch 2 does not match branch 1",
        ),
        (
            r#"{"tests":[{"sql":"WITH __source__raw_orders AS (SELECT 1 AS id), __expected__orders AS (SELECT 1 + 1) SELECT 1","fileLabel":"tests/orders.sql","mode":"model"}]}"#,
            "SQL test 'tests/orders.sql' must alias every non-trivial __expected__orders projection",
        ),
    ];
    for (request, expected_error) in cases {
        let error = extracted(request).expect_err("expected projection is rejected");
        assert_eq!(error, expected_error);
    }
    true
}

/// The extracted tests as JSON, or the message of the batch's first error.
pub(crate) fn extracted(request: &str) -> Result<String, String> {
    let response =
        batch_response(&serde_json::from_str(request).map_err(|error| error.to_string())?);
    response.get("error").map_or_else(
        || Ok(response["tests"].to_string()),
        |error| Err(error["message"].as_str().unwrap_or_default().to_owned()),
    )
}

fn extract_expected(
    mode: &str,
    actual: &str,
    expected_name: &str,
    expected_sql: &str,
) -> Result<String, String> {
    let sql = format!("WITH {actual}, {expected_name} AS ({expected_sql}) SELECT 1");
    extracted(
        &json!({"tests": [{"sql": sql, "fileLabel": "tests/orders.sql", "mode": mode}]})
            .to_string(),
    )
}

pub(crate) fn set_operation_expected_ctes_validate_every_branch() -> bool {
    let kinds = [
        (
            "model",
            "__source__raw_orders AS (SELECT 1 AS order_id)",
            "__expected__orders",
        ),
        (
            "macro",
            "__macro_actual__ AS (SELECT 1 AS order_id)",
            "__macro_expected__",
        ),
        (
            "udf",
            "__udf_actual__ AS (SELECT 1 AS order_id)",
            "__udf_expected__",
        ),
        (
            "table_fn",
            "__table_fn_actual__ AS (SELECT 1 AS order_id)",
            "__table_fn_expected__",
        ),
    ];
    let accepted = [
        "SELECT 1 AS order_id INTERSECT SELECT 1 AS order_id",
        "SELECT 1 AS order_id INTERSECT ALL SELECT 1 AS order_id",
        "SELECT 1 AS order_id INTERSECT DISTINCT SELECT 1 AS order_id",
        "SELECT 1 AS order_id EXCEPT SELECT 2 AS order_id",
        "SELECT 1 AS order_id EXCEPT ALL SELECT 2 AS order_id",
        "SELECT 1 AS order_id EXCEPT DISTINCT SELECT 2 AS order_id",
        "SELECT 1 AS order_id UNION SELECT 2 AS order_id EXCEPT SELECT 3 AS order_id",
        "SELECT 1 AS order_id INTERSECT /* EXCEPT */ -- UNION\n SELECT 1 AS order_id",
    ];
    for (mode, actual, expected_name) in kinds {
        for expected_sql in accepted {
            assert!(
                extract_expected(mode, actual, expected_name, expected_sql).is_ok(),
                "{mode}: {expected_sql}"
            );
        }
        let mismatch = format!(
            "SQL test 'tests/orders.sql' must use the same {expected_name} projection names and order in every set-operation branch; branch 2 does not match branch 1"
        );
        let third_mismatch = mismatch.replace("branch 2", "branch 3");
        let not_select = format!(
            "SQL test 'tests/orders.sql' must define each {expected_name} set-operation branch as a SELECT query"
        );
        let rejected = [
            (
                "SELECT 1 AS order_id INTERSECT SELECT 1 AS other_id",
                &mismatch,
            ),
            (
                "SELECT 1 AS order_id EXCEPT ALL SELECT 1 AS order_id, 2 AS extra",
                &mismatch,
            ),
            (
                "SELECT 1 AS order_id UNION SELECT 2 AS order_id EXCEPT SELECT 3 AS other_id",
                &third_mismatch,
            ),
            ("SELECT 1 AS order_id EXCEPT VALUES (1)", &not_select),
        ];
        for (expected_sql, expected_error) in rejected {
            assert_eq!(
                extract_expected(mode, actual, expected_name, expected_sql).as_ref(),
                Err(expected_error),
                "{mode}: {expected_sql}"
            );
        }
    }
    true
}

pub(crate) fn concurrent_requests_initialize_shared_template_once() -> bool {
    let templates = Arc::new(Mutex::new(HashMap::new()));
    let starts = Arc::new(Barrier::new(4));
    let initializations = Arc::new(AtomicUsize::new(0));
    let threads = (0..4)
        .map(|_| {
            let templates = Arc::clone(&templates);
            let starts = Arc::clone(&starts);
            let initializations = Arc::clone(&initializations);
            std::thread::spawn(move || {
                starts.wait();
                crate::compiler::_helpers::sql_tests::planning::cached_analysis_template(
                    "SELECT 1",
                    &templates,
                    || {
                        initializations.fetch_add(1, Ordering::Relaxed);
                        None
                    },
                )
                .expect("test assumption must hold")
            })
        })
        .collect::<Vec<_>>();

    for thread in threads {
        assert!(thread.join().expect("test assumption must hold").is_none());
    }
    assert_eq!(initializations.load(Ordering::Relaxed), 1);
    true
}

pub(crate) fn model_test_batch_returns_ordered_artifact() -> bool {
    let response: Value = serde_json::from_str(
        &plan_json(
            &json!({
                "lexicalSyntax": generic_lexical_syntax(),
                "models": [
                    {
                        "name": "stg_orders",
                        "querySql": "SELECT * FROM __source(\"raw_orders\")",
                        "modelDependencies": []
                    },
                    {
                        "name": "orders",
                        "querySql": "SELECT * FROM __ref(\"stg_orders\")",
                        "modelDependencies": ["stg_orders"]
                    }
                ],
                "functions": [],
                "tests": [{
                    "name": "orders_case",
                    "fileLabel": "tests/orders.sql",
                    "payload": {
                        "kind": "model",
                        "authoredCtes": [{
                            "name": "__source__raw_orders",
                            "sqlBody": "SELECT 1 AS order_id"
                        }],
                        "expectedCtes": [{
                            "name": "__expected__orders",
                            "sqlBody": "SELECT 1 AS order_id"
                        }],
                        "expectedModelNames": ["orders"],
                        "assertionCtes": []
                    }
                }],
                "sqlAnalysisEnabled": true,
                "sqlAnalysisDialect": "duckdb",
                "setDifferenceOperator": "EXCEPT",
                "workers": 2
            })
            .to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");

    assert_eq!(
        response["artifacts"][0]["modelNames"],
        json!(["stg_orders", "orders"])
    );
    let sql = response["artifacts"][0]["sql"]
        .as_str()
        .expect("test assumption must hold");
    assert!(sql.contains("__source__raw_orders AS (SELECT 1 AS order_id)"));
    assert!(sql.contains("__ref__stg_orders AS"));
    assert!(sql.contains("__actual__orders AS"));
    assert!(sql.contains("__expected__orders AS"));
    assert_eq!(response["artifacts"][0]["warnings"], json!([]));
    assert!(response["planningNs"].as_u64().is_some());
    assert!(response["renderingNs"].as_u64().is_some());
    true
}

pub(crate) fn unicode_cte_after_leading_with_preserves_identifier() -> bool {
    let response: Value = serde_json::from_str(
        &plan_json(
            &json!({
                "lexicalSyntax": generic_lexical_syntax(),
                "models": [{
                    "name": "orders",
                    "querySql": "WITH \"订单行\" AS (SELECT order_id FROM __source(\"raw_orders\")) SELECT order_id FROM \"订单行\"",
                    "modelDependencies": []
                }],
                "functions": [],
                "tests": [{
                    "name": "orders_case",
                    "fileLabel": "tests/orders.sql",
                    "payload": {
                        "kind": "model",
                        "authoredCtes": [{
                            "name": "__source__raw_orders",
                            "sqlBody": "SELECT 1 AS order_id"
                        }],
                        "expectedCtes": [{
                            "name": "__expected__orders",
                            "sqlBody": "SELECT 1 AS order_id"
                        }],
                        "expectedModelNames": ["orders"],
                        "assertionCtes": []
                    }
                }],
                "sqlAnalysisEnabled": true,
                "sqlAnalysisDialect": "duckdb"
            })
            .to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");

    let sql = response["artifacts"][0]["sql"]
        .as_str()
        .expect("test assumption must hold");
    assert!(sql.contains("WITH __source__raw_orders AS"));
    assert!(sql.contains("订单行"), "{sql}");
    true
}

pub(crate) fn unresolved_reference_fast_rejection_preserves_warning() -> bool {
    let response: Value = serde_json::from_str(
        &plan_json(
            &json!({
                "lexicalSyntax": generic_lexical_syntax(),
                "models": [{
                    "name": "orders",
                    "querySql": "SELECT * FROM __SOURCE(\"missing_orders\")",
                    "modelDependencies": []
                }],
                "tests": [{
                    "name": "orders_case",
                    "fileLabel": "tests/orders.sql",
                    "payload": {
                        "kind": "model",
                        "expectedModelNames": ["orders"]
                    }
                }],
                "sqlAnalysisEnabled": true,
                "sqlAnalysisDialect": "duckdb"
            })
            .to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");

    assert_eq!(
        response["artifacts"][0]["warnings"],
        json!([{
            "modelName": "orders",
            "severity": "error",
            "message": "test 'orders_case': model 'orders' references __source(\"missing_orders\") which has no mock"
        }])
    );
    true
}

pub(crate) fn ordered_comparison_batch_preserves_order() -> bool {
    let response: Value = serde_json::from_str(
        &crate::compiler::main::sql_test_rendering::render_json(
            &json!({
                "workers": 2,
                "requests": [{
                    "chain": [{
                        "modelName": "orders",
                        "resolvedSql": "SELECT 1 AS id",
                        "expectedCteSql": "SELECT 1 AS id"
                    }],
                    "assertions": [],
                    "sqlAnalysisEnabled": true,
                    "setDifferenceOperator": "EXCEPT",
                    "sqlAnalysisDialect": "duckdb"
                }]
            })
            .to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");

    let sql = response[0]["sql"]
        .as_str()
        .expect("test assumption must hold");
    assert!(
        sql.contains("__actual__orders AS (SELECT 1 AS id)"),
        "{sql}"
    );
    assert!(sql.contains("__expected__orders AS (SELECT 1 AS id)"));
    assert!(sql.contains("EXCEPT"));
    true
}

pub(crate) fn shared_textual_chain_renders_each_model_once() -> bool {
    const LAYERS: usize = 16;
    let mut models: Vec<Value> = vec![json!({
        "name": "orders_00",
        "querySql": "SELECT order_id, amount FROM __source(\"raw_orders\")",
        "modelDependencies": []
    })];
    for side in ["left", "right"] {
        models.push(diamond_model(1, side, "orders_00", "orders_00"));
    }
    for layer in 2..=LAYERS {
        let previous: String = format!("orders_{:02}_left", layer - 1);
        let other: String = format!("orders_{:02}_right", layer - 1);
        for side in ["left", "right"] {
            models.push(diamond_model(layer, side, &previous, &other));
        }
    }
    let top: String = format!("orders_{LAYERS:02}_left");
    let response: Value = serde_json::from_str(
        &plan_json(
            &json!({
                "lexicalSyntax": generic_lexical_syntax(),
                "models": models,
                "functions": [],
                "tests": [{
                    "name": "orders_totals",
                    "fileLabel": "tests/orders_totals.sql",
                    "payload": {
                        "kind": "model",
                        "authoredCtes": [{
                            "name": "__source__raw_orders",
                            "sqlBody": "SELECT 1 AS order_id, 1 AS amount"
                        }],
                        "expectedCtes": [{
                            "name": format!("__expected__{top}"),
                            "sqlBody": "SELECT 1 AS order_id, 65536 AS amount"
                        }],
                        "expectedModelNames": [top],
                        "assertionCtes": []
                    }
                }],
                "sqlAnalysisEnabled": false,
                "sqlAnalysisDialect": "duckdb",
                "setDifferenceOperator": "EXCEPT",
                "workers": 1
            })
            .to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");

    let sql = response["artifacts"][0]["sql"]
        .as_str()
        .expect("test assumption must hold");
    assert!(sql.len() < 50_000, "rendered {} bytes", sql.len());
    for layer in 1..LAYERS {
        for side in ["left", "right"] {
            let definition = format!("__ref__orders_{layer:02}_{side} AS (");
            assert_eq!(sql.matches(&definition).count(), 1, "{definition}");
        }
    }
    assert_eq!(sql.matches("__ref__orders_00 AS (").count(), 1);
    true
}

fn diamond_model(layer: usize, side: &str, previous: &str, other: &str) -> Value {
    json!({
        "name": format!("orders_{layer:02}_{side}"),
        "querySql": format!(
            "SELECT a.order_id, a.amount + b.amount AS amount \
             FROM __ref(\"{previous}\") AS a \
             INNER JOIN __ref(\"{other}\") AS b ON a.order_id = b.order_id"
        ),
        "modelDependencies": [previous, other]
    })
}

fn deep_diamond_models(layers: usize) -> Vec<Value> {
    let mut models: Vec<Value> = vec![json!({
        "name": "orders_00",
        "querySql": "SELECT o.order_id, o.amount FROM __source(\"raw_orders\") AS o \
                     INNER JOIN __source(\"raw_customers\") AS c ON o.customer_id = c.customer_id",
        "modelDependencies": []
    })];
    for side in ["left", "right"] {
        models.push(diamond_model(1, side, "orders_00", "orders_00"));
    }
    for layer in 2..=layers {
        let previous: String = format!("orders_{:02}_left", layer - 1);
        let other: String = format!("orders_{:02}_right", layer - 1);
        for side in ["left", "right"] {
            models.push(diamond_model(layer, side, &previous, &other));
        }
    }
    models
}

fn plan_deep_diamond_with_missing_mock(sql_analysis_enabled: bool) -> Value {
    const LAYERS: usize = 12;
    let top: String = format!("orders_{LAYERS:02}_left");
    serde_json::from_str(
        &plan_json(
            &json!({
                "lexicalSyntax": generic_lexical_syntax(),
                "models": deep_diamond_models(LAYERS),
                "tests": [{
                    "name": "orders_totals",
                    "fileLabel": "tests/orders_totals.sql",
                    "payload": {
                        "kind": "model",
                        "authoredCtes": [{
                            "name": "__source__raw_customers",
                            "sqlBody": "SELECT 1 AS customer_id"
                        }],
                        "expectedCtes": [{
                            "name": format!("__expected__{top}"),
                            "sqlBody": "SELECT 1 AS order_id, 1 AS amount"
                        }],
                        "expectedModelNames": [top],
                    }
                }],
                "sqlAnalysisEnabled": sql_analysis_enabled,
                "sqlAnalysisDialect": "duckdb",
                "renderSql": false
            })
            .to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold")
}

pub(crate) fn deep_shared_graph_reports_missing_mock_once() -> bool {
    for sql_analysis_enabled in [true, false] {
        let response = plan_deep_diamond_with_missing_mock(sql_analysis_enabled);
        assert_eq!(
            response["artifacts"][0]["warnings"],
            json!([{
                "modelName": "orders_00",
                "severity": "error",
                "message": "test 'orders_totals': model 'orders_00' references __source(\"raw_orders\") which has no mock"
            }]),
            "sql_analysis_enabled={sql_analysis_enabled}"
        );
    }
    true
}

pub(crate) fn plan_without_rendering_returns_executable_steps() -> bool {
    let response: Value = serde_json::from_str(
        &plan_json(
            &json!({
                "lexicalSyntax": generic_lexical_syntax(),
                "models": [
                    {
                        "name": "stg_orders",
                        "querySql": "SELECT * FROM __source(\"raw_orders\")",
                        "modelDependencies": []
                    },
                    {
                        "name": "orders",
                        "querySql": "SELECT * FROM __ref(\"stg_orders\")",
                        "modelDependencies": ["stg_orders"]
                    }
                ],
                "tests": [{
                    "name": "orders_case",
                    "fileLabel": "tests/orders.sql",
                    "payload": {
                        "kind": "model",
                        "authoredCtes": [{
                            "name": "__source__raw_orders",
                            "sqlBody": "SELECT 1 AS order_id"
                        }],
                        "expectedCtes": [{
                            "name": "__expected__orders",
                            "sqlBody": "SELECT 1 AS order_id"
                        }],
                        "expectedModelNames": ["orders"],
                        "assertionCtes": [{
                            "name": "__assert__positive",
                            "sqlBody": "SELECT * FROM __ref(\"orders\") WHERE order_id < 0"
                        }]
                    }
                }],
                "sqlAnalysisEnabled": false,
                "sqlAnalysisDialect": "duckdb",
                "renderSql": false
            })
            .to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");

    let artifact = &response["artifacts"][0];
    assert_eq!(artifact["sql"], Value::Null);
    assert_eq!(artifact["chain"][0]["modelName"], json!("stg_orders"));
    assert_eq!(artifact["chain"][0]["expectedCteSql"], Value::Null);
    assert_eq!(
        artifact["chain"][1]["liftedCtes"],
        json!([
            ["__source__raw_orders", "SELECT 1 AS order_id"],
            ["__ref__stg_orders", "SELECT * FROM __source__raw_orders"]
        ])
    );
    assert_eq!(
        artifact["chain"][1]["comparisonBodySql"],
        json!("SELECT * FROM __ref__stg_orders")
    );
    assert_eq!(
        artifact["chain"][1]["expectedCteSql"],
        json!("SELECT 1 AS order_id")
    );
    assert_eq!(artifact["assertions"][0]["name"], json!("positive"));
    assert_eq!(
        artifact["assertions"][0]["comparisonBodySql"],
        json!("SELECT * FROM __ref__orders WHERE order_id < 0")
    );
    assert_eq!(
        artifact["assertions"][0]["liftedCtes"][2],
        json!(["__ref__orders", "SELECT * FROM __ref__stg_orders"])
    );
    assert!(
        artifact["assertions"][0]["resolvedSql"]
            .as_str()
            .is_some_and(|sql| sql.contains("__ref__orders AS (SELECT * FROM __ref__stg_orders)"))
    );
    true
}

pub(crate) fn textual_assertion_with_clause_merges_lifted_ctes() -> bool {
    let response: Value = serde_json::from_str(
        &plan_json(
            &json!({
                "lexicalSyntax": generic_lexical_syntax(),
                "models": [
                    {
                        "name": "stg_orders",
                        "querySql": "SELECT * FROM __source(\"raw_orders\")",
                        "modelDependencies": []
                    },
                    {
                        "name": "orders",
                        "querySql": "SELECT * FROM __ref(\"stg_orders\")",
                        "modelDependencies": ["stg_orders"]
                    }
                ],
                "tests": [{
                    "name": "orders_case",
                    "fileLabel": "tests/orders.sql",
                    "payload": {
                        "kind": "model",
                        "authoredCtes": [{
                            "name": "__source__raw_orders",
                            "sqlBody": "SELECT 1 AS order_id"
                        }],
                        "expectedCtes": [],
                        "expectedModelNames": [],
                        "assertionCtes": [{
                            "name": "__assert__no_negative_orders",
                            "sqlBody": "WITH negative_orders AS (SELECT * FROM __ref(\"orders\") WHERE order_id < 0) SELECT * FROM negative_orders"
                        }]
                    }
                }],
                "sqlAnalysisEnabled": false,
                "sqlAnalysisDialect": "duckdb",
                "renderSql": false
            })
            .to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");

    let resolved_sql = response["artifacts"][0]["assertions"][0]["resolvedSql"]
        .as_str()
        .expect("resolved assertion SQL");
    assert_eq!(
        resolved_sql,
        concat!(
            "WITH __source__raw_orders AS (SELECT 1 AS order_id), ",
            "__ref__stg_orders AS (SELECT * FROM __source__raw_orders), ",
            "__ref__orders AS (SELECT * FROM __ref__stg_orders), ",
            "negative_orders AS (SELECT * FROM __ref__orders WHERE order_id < 0) ",
            "SELECT * FROM negative_orders"
        )
    );
    assert!(
        sql_test_parser(Some("duckdb"))
            .expect("DuckDB parser")
            .read_whole(resolved_sql)
            .is_ok(),
        "{resolved_sql}"
    );
    true
}

pub(crate) fn upstream_fallback_resolves() -> bool {
    let request = json!({
        "lexicalSyntax": generic_lexical_syntax(),
        "models": [
            {"name": "stg_orders", "querySql": "SELECT __udf(\"identity_value\")(order_id) AS order_id FROM __source(\"raw_orders\")"},
            {"name": "orders", "querySql": "SELECT * FROM __ref(\"stg_orders\")", "modelDependencies": ["stg_orders"]}
        ],
        "functions": [{"name": "identity_value", "udfPrefix": "identity_value(", "udfSuffix": ")"}],
        "tests": [{
            "name": "orders_case", "fileLabel": "tests/orders.sql",
            "payload": {
                "kind": "model",
                "authoredCtes": [{"name": "__source__raw_orders", "sqlBody": "SELECT 1 AS order_id"}],
                "expectedCtes": [{"name": "__expected__orders", "sqlBody": "SELECT 1 AS order_id"}],
                "expectedModelNames": ["orders"],
                "assertionCtes": [{"name": "__assert__positive", "sqlBody": "SELECT * FROM __ref(\"orders\") WHERE order_id < 0"}]
            }
        }],
        "sqlAnalysisEnabled": true, "sqlAnalysisDialect": "duckdb"
    });
    let response: Value =
        serde_json::from_str(&plan_json(&request.to_string()).expect("planning succeeds"))
            .expect("valid JSON");
    let artifact = &response["artifacts"][0];
    assert_eq!(artifact["warnings"], json!([]));
    let sql = artifact["sql"].as_str().expect("rendered SQL");
    assert!(!sql.contains("__ref(\""));
    assert!(sql.contains("identity_value((order_id))"), "{sql}");
    assert!(sql.contains("__ref__stg_orders"));
    let mut artifact_request = request;
    artifact_request["includePlan"] = json!(false);
    let compact: Value = serde_json::from_str(
        &plan_json(&artifact_request.to_string()).expect("artifact planning succeeds"),
    )
    .expect("valid artifact JSON");
    assert_eq!(compact["artifacts"][0]["sql"], artifact["sql"]);
    assert_eq!(
        compact["artifacts"][0]["modelNames"],
        artifact["modelNames"]
    );
    assert_eq!(compact["artifacts"][0]["warnings"], artifact["warnings"]);
    assert_eq!(compact["artifacts"][0]["chain"], json!([]));
    assert_eq!(compact["artifacts"][0]["assertions"], json!([]));
    true
}

pub(crate) fn chain_resolution_orders_unmocked_models() -> bool {
    let response: Value = serde_json::from_str(
        &chain_json(
            &json!({
                "lexicalSyntax": generic_lexical_syntax(),
                "models": deep_diamond_models(2),
                "tests": [
                    {
                        "name": "orders_totals",
                        "fileLabel": "tests/orders_totals.sql",
                        "payload": {
                            "kind": "model",
                            "authoredCtes": [{
                                "name": "__ref__orders_01_right",
                                "sqlBody": "SELECT 1 AS order_id, 1 AS amount"
                            }],
                            "expectedModelNames": ["orders_02_left"]
                        }
                    },
                    {
                        "name": "direct_case",
                        "fileLabel": "tests/direct.sql",
                        "payload": {
                            "kind": "direct",
                            "mode": "macro",
                            "actualCte": {"name": "__macro_actual__", "sqlBody": "SELECT 1"},
                            "expectedCte": {"name": "__macro_expected__", "sqlBody": "SELECT 1"}
                        }
                    }
                ]
            })
            .to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");

    assert_eq!(
        response["chains"],
        json!([["orders_00", "orders_01_left", "orders_02_left"], []])
    );
    true
}

pub(crate) fn difference_sample_lifts_generated_ctes_and_bounds_rows() -> bool {
    let request = |use_top_clause: bool| {
        json!({
            "step": {
                "modelName": "orders",
                "resolvedSql": "WITH __ref__stg_orders AS (SELECT 1 AS order_id) SELECT * FROM __ref__stg_orders",
                "expectedCteSql": "SELECT 2 AS order_id",
                "liftedCtes": [["__ref__stg_orders", "SELECT 1 AS order_id"]],
                "comparisonBodySql": "SELECT * FROM __ref__stg_orders"
            },
            "sqlAnalysisEnabled": false,
            "setDifferenceOperator": "EXCEPT",
            "sqlAnalysisDialect": "duckdb",
            "direction": "missing",
            "sampleLimit": 3,
            "useTopClause": use_top_clause
        })
        .to_string()
    };
    let limited: Value = serde_json::from_str(
        &crate::compiler::main::sql_test_difference_sampling::render_difference_sample_json(
            &request(false),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");
    let top: Value = serde_json::from_str(
        &crate::compiler::main::sql_test_difference_sampling::render_difference_sample_json(
            &request(true),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");

    assert_eq!(
        limited["sql"],
        json!(
            "WITH __ref__stg_orders AS (SELECT 1 AS order_id),\n\
             __ref__orders AS (SELECT * FROM __ref__stg_orders),\n\
             __actual AS (SELECT * FROM __ref__orders),\n\
             __expected AS (SELECT 2 AS order_id)\n\
             SELECT * FROM (SELECT * FROM __expected EXCEPT SELECT * FROM __actual) \
             AS __sqlbuild_difference LIMIT 3"
        )
    );
    assert!(
        top["sql"]
            .as_str()
            .is_some_and(|sql| sql.contains("SELECT TOP 3 * FROM (") && !sql.contains("LIMIT"))
    );
    true
}

fn step_sql_bytes(step: &Value) -> usize {
    let text_len = |value: &Value| value.as_str().map_or(0, str::len);
    let lifted: usize = step["liftedCtes"].as_array().map_or(0, |ctes| {
        ctes.iter()
            .map(|cte| text_len(&cte[0]) + text_len(&cte[1]))
            .sum()
    });
    text_len(&step["resolvedSql"]) + text_len(&step["comparisonBodySql"]) + lifted
}

pub(crate) fn long_chain_plan_output_stays_linear() -> bool {
    const MODELS: usize = 200;
    let mut models: Vec<Value> = vec![json!({
        "name": "orders_000",
        "querySql": "SELECT order_id, amount FROM __source(\"raw_orders\")",
        "modelDependencies": []
    })];
    for index in 1..MODELS {
        let previous: String = format!("orders_{:03}", index - 1);
        models.push(json!({
            "name": format!("orders_{index:03}"),
            "querySql": format!(
                "SELECT order_id, amount + 1 AS amount FROM __ref(\"{previous}\")"
            ),
            "modelDependencies": [previous]
        }));
    }
    let tip: String = format!("orders_{:03}", MODELS - 1);
    for sql_analysis_enabled in [true, false] {
        let response: Value = serde_json::from_str(
            &plan_json(
                &json!({
                    "lexicalSyntax": generic_lexical_syntax(),
                    "models": models,
                    "tests": [{
                        "name": "orders_tip",
                        "fileLabel": "tests/orders_tip.sql",
                        "payload": {
                            "kind": "model",
                            "authoredCtes": [{
                                "name": "__source__raw_orders",
                                "sqlBody": "SELECT 1 AS order_id, 0 AS amount"
                            }],
                            "expectedCtes": [{
                                "name": format!("__expected__{tip}"),
                                "sqlBody": "SELECT 1 AS order_id, 199 AS amount"
                            }],
                            "expectedModelNames": [tip],
                        }
                    }],
                    "sqlAnalysisEnabled": sql_analysis_enabled,
                    "sqlAnalysisDialect": "duckdb"
                })
                .to_string(),
            )
            .expect("test assumption must hold"),
        )
        .expect("test assumption must hold");
        let artifact = &response["artifacts"][0];
        let sql = artifact["sql"].as_str().expect("test assumption must hold");
        let chain = artifact["chain"]
            .as_array()
            .expect("test assumption must hold");
        assert_eq!(chain.len(), MODELS);
        let step_bytes: usize = chain.iter().map(step_sql_bytes).sum();
        assert!(
            step_bytes <= 3 * sql.len(),
            "step SQL {step_bytes} bytes for {} rendered bytes",
            sql.len()
        );
        assert!(
            chain[..MODELS - 1]
                .iter()
                .all(|step| step_sql_bytes(step) == 0)
        );

        let mut padded_chain: Vec<Value> = chain.clone();
        for step in &mut padded_chain[..MODELS - 1] {
            step["resolvedSql"] = json!("SELECT unrendered_column FROM unrendered_relation");
            step["liftedCtes"] = json!([["unrendered_cte", "SELECT 1"]]);
            step["comparisonBodySql"] = json!("SELECT unrendered_column");
        }
        let rendered: Value = serde_json::from_str(
            &crate::compiler::main::sql_test_rendering::render_json(
                &json!({
                    "requests": [{
                        "chain": padded_chain,
                        "assertions": [],
                        "sqlAnalysisEnabled": sql_analysis_enabled,
                        "setDifferenceOperator": "EXCEPT",
                        "sqlAnalysisDialect": "duckdb"
                    }]
                })
                .to_string(),
            )
            .expect("test assumption must hold"),
        )
        .expect("test assumption must hold");
        assert_eq!(rendered[0]["sql"].as_str(), Some(sql));
    }
    true
}

fn render_single(request: Value) -> String {
    let response: Value = serde_json::from_str(
        &crate::compiler::main::sql_test_rendering::render_json(
            &json!({"requests": [request]}).to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");
    response[0]["sql"]
        .as_str()
        .expect("test assumption must hold")
        .to_string()
}

fn partial_expected_step() -> Value {
    json!({
        "modelName": "orders",
        "resolvedSql": "SELECT 1 AS order_id, 'paid' AS status, 10 AS amount",
        "expectedCteSql": "SELECT 'paid' AS status, 1 AS order_id",
        "expectedColumns": ["status", "order_id"]
    })
}

pub(crate) fn partial_expected_columns_project_both_sides() -> bool {
    let sql = render_single(json!({
        "chain": [partial_expected_step()],
        "sqlAnalysisEnabled": false,
        "sqlAnalysisDialect": "duckdb"
    }));
    assert!(sql.contains("(SELECT COUNT(*) FROM __actual__orders) AS actual_count"));
    assert!(sql.contains(
        "(SELECT status, order_id FROM __actual__orders EXCEPT SELECT status, order_id FROM __expected__orders) AS __sqlbuild_mismatch"
    ), "{sql}");
    assert!(sql.contains(
        "(SELECT status, order_id FROM __expected__orders EXCEPT SELECT status, order_id FROM __actual__orders) AS __sqlbuild_missing"
    ), "{sql}");
    assert!(!sql.contains("SELECT * FROM __actual__orders"));
    true
}

pub(crate) fn actual_probe_selects_zero_rows_from_step() -> bool {
    let sql = render_single(json!({
        "chain": [partial_expected_step()],
        "sqlAnalysisEnabled": false,
        "sqlAnalysisDialect": "duckdb",
        "probeStepIndex": 0
    }));
    assert!(sql.starts_with("WITH __actual__orders AS ("), "{sql}");
    assert!(
        sql.ends_with("\nSELECT * FROM __actual__orders WHERE 1 = 0"),
        "{sql}"
    );
    assert!(!sql.contains("UNION ALL"));
    true
}

pub(crate) fn sqlserver_difference_sample_projects_bracketed_columns() -> bool {
    let response: Value = serde_json::from_str(
        &crate::compiler::main::sql_test_difference_sampling::render_difference_sample_json(
            &json!({
                "step": {
                    "modelName": "orders",
                    "resolvedSql": "SELECT 1 AS [Order Id], 10 AS amount, 'paid' AS status",
                    "expectedCteSql": "SELECT 1 AS [Order Id], 10 AS amount",
                    "expectedColumns": ["[Order Id]", "amount"]
                },
                "sqlAnalysisEnabled": true,
                "setDifferenceOperator": "EXCEPT",
                "sqlAnalysisDialect": "tsql",
                "direction": "unexpected",
                "sampleLimit": 3,
                "useTopClause": true
            })
            .to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");
    let sql = response["sql"].as_str().expect("test assumption must hold");
    assert!(sql.contains(
        "SELECT TOP 3 * FROM (SELECT [Order Id], amount FROM __actual EXCEPT SELECT [Order Id], amount FROM __expected) AS __sqlbuild_difference"
    ), "{sql}");
    true
}

pub(crate) fn snowflake_plan_keeps_quoted_expected_columns() -> bool {
    let response: Value = serde_json::from_str(
        &plan_json(
            &json!({
                "lexicalSyntax": generic_lexical_syntax(),
                "models": [{
                    "name": "orders",
                    "querySql": "SELECT order_id AS \"Order Id\", status, amount FROM __source(\"raw_orders\")",
                    "modelDependencies": []
                }],
                "tests": [{
                    "name": "orders_case",
                    "fileLabel": "tests/orders.sql",
                    "payload": {
                        "kind": "model",
                        "authoredCtes": [{
                            "name": "__source__raw_orders",
                            "sqlBody": "SELECT 1 AS order_id, 'paid' AS status, 10 AS amount"
                        }],
                        "expectedCtes": [{
                            "name": "__expected__orders",
                            "sqlBody": "SELECT 'paid' AS status, 1 AS \"Order Id\""
                        }],
                        "expectedModelNames": ["orders"]
                    }
                }],
                "sqlAnalysisEnabled": false,
                "sqlAnalysisDialect": "snowflake"
            })
            .to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");
    let artifact = &response["artifacts"][0];
    assert_eq!(
        artifact["chain"][0]["expectedColumns"],
        json!(["status", "\"Order Id\""])
    );
    let sql = artifact["sql"].as_str().expect("test assumption must hold");
    assert!(sql.contains(
        "SELECT status, \"Order Id\" FROM __actual__orders EXCEPT SELECT status, \"Order Id\" FROM __expected__orders"
    ), "{sql}");
    true
}

fn plan_helper_scope_request(
    sql_analysis_enabled: bool,
    helper_name: &str,
) -> Result<String, String> {
    plan_json(
        &json!({
            "lexicalSyntax": generic_lexical_syntax(),
            "models": [
                {
                    "name": "orders",
                    "querySql": "SELECT order_id, amount FROM __ref(\"stg_orders\")",
                    "modelDependencies": ["stg_orders"]
                }
            ],
            "tests": [{
                "name": "orders_case",
                "fileLabel": "tests/orders.sql",
                "payload": {
                    "kind": "model",
                    "authoredCtes": [
                        {"name": "__ref__stg_orders", "sqlBody": "SELECT 1 AS order_id, 10 AS amount"},
                        {"name": "expected_rows", "sqlBody": "SELECT 1 AS order_id, 10 AS amount"},
                        {"name": helper_name, "sqlBody": "SELECT s.order_id FROM __ref__stg_orders AS s JOIN expected_rows AS e ON e.order_id = s.order_id"},
                        {"name": "unused_rows", "sqlBody": "SELECT 3 AS order_id"}
                    ],
                    "expectedCtes": [{
                        "name": "__expected__orders",
                        "sqlBody": "SELECT order_id, amount FROM expected_rows"
                    }],
                    "expectedModelNames": ["orders"],
                    "assertionCtes": [{
                        "name": "__assert__ids_match",
                        "sqlBody": format!("SELECT order_id FROM __ref(\"orders\") EXCEPT SELECT order_id FROM {helper_name}")
                    }]
                }
            }],
            "sqlAnalysisEnabled": sql_analysis_enabled,
            "sqlAnalysisDialect": "duckdb",
            "setDifferenceOperator": "EXCEPT"
        })
        .to_string(),
    )
}

fn top_level_cte_position(sql: &str, name: &str) -> usize {
    let needle = format!("{name} AS (");
    let positions: Vec<usize> = sql
        .match_indices(&needle)
        .map(|(index, _)| index)
        .filter(|index| *index == 5 || sql[..*index].ends_with(",\n"))
        .collect();
    assert_eq!(
        positions.len(),
        1,
        "{name} must be defined once at top level: {sql}"
    );
    positions[0]
}

/// Case-folded names of the CTEs a rendered test query defines at its top level.
pub(crate) fn top_level_cte_keys(sql: &str, dialect: &str) -> Vec<String> {
    let dialect =
        crate::compiler::_helpers::sql_tests::cte_slices::SliceDialect::new(Some(dialect));
    crate::compiler::_helpers::sql_tests::cte_slices::split_top_level_with(sql, dialect)
        .expect("rendered SQL scans")
        .expect("rendered SQL has a WITH")
        .ctes
        .iter()
        .map(|cte| cte.key.clone())
        .collect()
}

fn assert_rendered_shape(sql: &str, shape: &RenderedShapeTestCase, helpers: &[&str]) {
    let top_level = top_level_cte_keys(sql, "duckdb");
    for helper in helpers {
        assert!(!top_level.contains(&(*helper).to_string()), "{sql}");
    }
    for (first, second) in shape.expected_order {
        assert!(
            top_level_cte_position(sql, first) < top_level_cte_position(sql, second),
            "{}: {first} before {second}: {sql}",
            shape.description
        );
    }
    for fragment in shape.expected_fragments {
        assert!(
            sql.contains(fragment),
            "{}: {fragment}: {sql}",
            shape.description
        );
    }
    for fragment in shape.expected_absent_fragments {
        assert!(
            !sql.contains(fragment),
            "{}: {fragment}: {sql}",
            shape.description
        );
    }
}

pub(crate) fn helper_ctes_are_in_scope_for_assertions_and_expected_rows() -> bool {
    let shapes = [
        RenderedShapeTestCase {
            description: "sql analysis on",
            sql_analysis_enabled: true,
            expected_order: &[
                ("__ref__stg_orders", "__expected__orders"),
                ("__expected__orders", "__assert__ids_match"),
                ("__ref__stg_orders", "__helper__matched_ids"),
                ("__helper__expected_rows", "__helper__matched_ids"),
                ("__helper__expected_rows", "__expected__orders"),
                ("__helper__matched_ids", "__assert__ids_match"),
            ],
            expected_fragments: &[
                "__expected__orders AS (SELECT order_id, amount FROM __helper__expected_rows AS expected_rows)",
            ],
            expected_absent_fragments: &["__helper__unused_rows"],
        },
        RenderedShapeTestCase {
            description: "sql analysis off",
            sql_analysis_enabled: false,
            expected_order: &[
                ("__ref__stg_orders", "__expected__orders"),
                ("__expected__orders", "__assert__ids_match"),
            ],
            expected_fragments: &[
                "__expected__orders AS (WITH expected_rows AS (SELECT 1 AS order_id, 10 AS amount) \
                 SELECT order_id, amount FROM expected_rows)",
            ],
            expected_absent_fragments: &["unused_rows"],
        },
    ];
    for shape in shapes {
        let response: Value = serde_json::from_str(
            &plan_helper_scope_request(shape.sql_analysis_enabled, "matched_ids")
                .expect("test assumption must hold"),
        )
        .expect("test assumption must hold");
        let sql = response["artifacts"][0]["sql"]
            .as_str()
            .expect("test assumption must hold");
        assert_rendered_shape(
            sql,
            &shape,
            &["expected_rows", "matched_ids", "unused_rows"],
        );
        assert_eq!(response["artifacts"][0]["warnings"], json!([]), "{sql}");
    }
    true
}

pub(crate) fn scoped_helper_named_like_generated_cte_is_rejected() -> bool {
    let error =
        plan_helper_scope_request(true, "__actual__orders").expect_err("test assumption must hold");
    assert_eq!(
        error,
        "compile_input:SQL test 'tests/orders.sql' defines CTE '__actual__orders', which conflicts with the generated CTE"
    );
    true
}

pub(crate) fn difference_sample_lifts_expected_helper_ctes() -> bool {
    let response: Value = serde_json::from_str(
        &crate::compiler::main::sql_test_difference_sampling::render_difference_sample_json(
            &json!({
                "step": {
                    "modelName": "orders",
                    "resolvedSql": "SELECT 1 AS order_id",
                    "expectedCteSql": "SELECT order_id FROM expected_rows",
                    "expectedLiftedCtes": [["expected_rows", "SELECT 2 AS order_id"]],
                    "expectedColumns": ["order_id"]
                },
                "sqlAnalysisEnabled": false,
                "setDifferenceOperator": "EXCEPT",
                "sqlAnalysisDialect": "duckdb",
                "direction": "missing",
                "sampleLimit": 3
            })
            .to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");
    assert_eq!(
        response["sql"],
        json!(
            "WITH expected_rows AS (SELECT 2 AS order_id),\n\
             __actual AS (SELECT 1 AS order_id),\n\
             __expected AS (SELECT order_id FROM expected_rows)\n\
             SELECT * FROM (SELECT order_id FROM __expected EXCEPT SELECT order_id FROM __actual) \
             AS __sqlbuild_difference LIMIT 3"
        )
    );
    true
}

pub(crate) fn mock_read_through_helper_brings_its_mock_dependencies_into_scope() -> bool {
    let shapes = [
        RenderedShapeTestCase {
            description: "sql analysis on",
            sql_analysis_enabled: true,
            expected_order: &[
                ("__ref__raw_orders", "__ref__stg_orders"),
                ("__ref__raw_orders", "__actual__orders"),
                ("__ref__raw_orders", "__helper__base_rows"),
                ("__helper__base_rows", "__ref__stg_orders"),
                ("__ref__stg_orders", "__helper__expected_rows"),
                ("__helper__expected_rows", "__assert__rows_match"),
            ],
            expected_fragments: &[],
            expected_absent_fragments: &[],
        },
        RenderedShapeTestCase {
            description: "sql analysis off",
            sql_analysis_enabled: false,
            expected_order: &[
                ("__ref__raw_orders", "__ref__stg_orders"),
                ("__ref__raw_orders", "__actual__orders"),
            ],
            expected_fragments: &[
                "__ref__stg_orders AS (WITH base_rows AS (SELECT order_id FROM __ref__raw_orders) \
                 SELECT order_id FROM base_rows)",
            ],
            expected_absent_fragments: &[],
        },
    ];
    for shape in shapes {
        let response: Value = serde_json::from_str(
            &plan_json(
                &json!({
                    "lexicalSyntax": generic_lexical_syntax(),
                    "models": [
                        {"name": "raw_orders", "querySql": "SELECT 1 AS order_id", "modelDependencies": []},
                        {"name": "stg_orders", "querySql": "SELECT order_id FROM __ref(\"raw_orders\")", "modelDependencies": ["raw_orders"]},
                        {"name": "orders", "querySql": "SELECT order_id FROM __ref(\"stg_orders\")", "modelDependencies": ["stg_orders"]}
                    ],
                    "tests": [{
                        "name": "orders_case",
                        "fileLabel": "tests/orders.sql",
                        "payload": {
                            "kind": "model",
                            "authoredCtes": [
                                {"name": "__ref__raw_orders", "sqlBody": "SELECT 1 AS order_id"},
                                {"name": "base_rows", "sqlBody": "SELECT order_id FROM __ref__raw_orders"},
                                {"name": "__ref__stg_orders", "sqlBody": "SELECT order_id FROM base_rows"},
                                {"name": "expected_rows", "sqlBody": "SELECT order_id FROM __ref__stg_orders"}
                            ],
                            "expectedCtes": [{
                                "name": "__expected__orders",
                                "sqlBody": "SELECT order_id FROM expected_rows"
                            }],
                            "expectedModelNames": ["orders"],
                            "assertionCtes": [{
                                "name": "__assert__rows_match",
                                "sqlBody": "SELECT order_id FROM expected_rows EXCEPT SELECT order_id FROM __ref(\"orders\")"
                            }]
                        }
                    }],
                    "sqlAnalysisEnabled": shape.sql_analysis_enabled,
                    "sqlAnalysisDialect": "duckdb",
                    "setDifferenceOperator": "EXCEPT"
                })
                .to_string(),
            )
            .expect("test assumption must hold"),
        )
        .expect("test assumption must hold");
        let sql = response["artifacts"][0]["sql"]
            .as_str()
            .expect("test assumption must hold");
        assert_rendered_shape(sql, &shape, &["base_rows", "expected_rows"]);
        assert_eq!(response["artifacts"][0]["warnings"], json!([]), "{sql}");
    }
    true
}

fn render_one(request: Value) -> Result<String, String> {
    let response = crate::compiler::_helpers::sql_tests::rendering::render_json(
        &json!({"requests": [request]}).to_string(),
    )?;
    let response: Vec<Value> = serde_json::from_str(&response).expect("render response");
    Ok(response[0]["sql"]
        .as_str()
        .expect("rendered query")
        .to_string())
}

fn colliding_chain_request(dialect: &str, name: &str) -> Value {
    json!({
        "sqlAnalysisDialect": dialect,
        "chain": [{
            "modelName": "orders",
            "resolvedSql": format!("WITH {name} AS (SELECT 2 AS order_id) SELECT final.order_id FROM final"),
            "comparisonBodySql": format!("WITH {name} AS (SELECT order_id + 1 AS order_id FROM __ref__stg_orders) SELECT final.order_id FROM final"),
            "liftedCtes": [["__ref__stg_orders", format!("WITH {name} AS (SELECT 1 AS order_id) SELECT * FROM {name}")]],
            "expectedCteSql": "SELECT 2 AS order_id"
        }]
    })
}

pub(super) fn repeated_model_sql_renders_like_separate_batches() -> bool {
    let step = |reserved: &str| {
        json!({
            "sqlAnalysisDialect": "duckdb",
            "chain": [{
                "modelName": "orders",
                "resolvedSql": "WITH final AS (SELECT 2 AS order_id) SELECT final.order_id FROM final",
                "expectedCteSql": format!("SELECT 2 AS order_id {reserved}")
            }]
        })
    };
    let render = |requests: Vec<Value>| -> Vec<Value> {
        let response = crate::compiler::_helpers::sql_tests::rendering::render_json(
            &json!({"requests": requests}).to_string(),
        )
        .expect("render batch");
        serde_json::from_str::<Vec<Value>>(&response).expect("render response")
    };
    let plain = step("");
    let commented = step("-- trailing comment");
    let separate: Vec<Value> = [&plain, &commented, &plain]
        .into_iter()
        .flat_map(|request| render(vec![request.clone()]))
        .collect();
    let batched = render(vec![plain.clone(), commented.clone(), plain]);
    separate == batched
        && batched[0] != batched[1]
        && batched[0]["sql"].as_str().is_some_and(|sql| {
            sql.starts_with("WITH final AS (SELECT 2 AS order_id),\n")
                && sql.contains("__actual__orders AS (SELECT final.order_id FROM final)")
        })
}

pub(super) fn shared_model_cte_names_stay_nested_on_nested_with_dialects() -> bool {
    [
        ("duckdb", "final"),
        ("snowflake", "final"),
        ("postgres", "final"),
        ("bigquery", "`final`"),
        ("databricks", "`final`"),
    ]
    .iter()
    .all(|(dialect, name)| {
        let upstream = format!("WITH {name} AS (SELECT 1 AS order_id) SELECT * FROM {name}");
        let downstream = format!(
            "WITH {name} AS (SELECT order_id + 1 AS order_id FROM __ref__stg_orders) SELECT final.order_id FROM final"
        );
        let sql = render_one(json!({
            "sqlAnalysisDialect": dialect,
            "chain": [
                {
                    "modelName": "stg_orders",
                    "resolvedSql": upstream,
                    "comparisonBodySql": upstream,
                    "expectedCteSql": "SELECT 1 AS order_id"
                },
                {
                    "modelName": "orders",
                    "resolvedSql": format!("WITH __ref__stg_orders AS ({upstream}), {}", &downstream[5..]),
                    "comparisonBodySql": downstream,
                    "liftedCtes": [["__ref__stg_orders", upstream]],
                    "expectedCteSql": "SELECT 2 AS order_id"
                }
            ]
        }))
        .expect("nested model scopes");
        sql.starts_with(&format!(
            "WITH __ref__stg_orders AS ({upstream}),\n\
             __ref__orders AS ({downstream}),\n\
             __actual__stg_orders AS (SELECT * FROM __ref__stg_orders),\n"
        )) && sql.contains("__actual__orders AS (SELECT * FROM __ref__orders)")
            && sql.matches(&upstream).count() == 1
            && sql.matches(&downstream).count() == 1
    })
}

pub(super) fn generated_with_bodies_stay_nested_verbatim() -> bool {
    let sql = render_one(colliding_chain_request("duckdb", "final")).expect("render");
    sql.starts_with(
        "WITH __ref__stg_orders AS (WITH final AS (SELECT 1 AS order_id) SELECT * FROM final),\n\
         __ref__orders AS (WITH final AS (SELECT order_id + 1 AS order_id FROM __ref__stg_orders) \
         SELECT final.order_id FROM final),\n\
         __actual__orders AS (SELECT * FROM __ref__orders),\n",
    )
}

pub(super) fn tsql_model_cte_collisions_are_renamed_by_token_span() -> bool {
    let sql = render_one(colliding_chain_request("tsql", "final")).expect("T-SQL rename");
    sql.starts_with(
        "WITH final AS (SELECT 1 AS order_id),\n\
         __ref__stg_orders AS (SELECT * FROM final),\n\
         __sqb_cte_0 AS (SELECT order_id + 1 AS order_id FROM __ref__stg_orders),\n\
         __ref__orders AS (SELECT __sqb_cte_0.order_id FROM __sqb_cte_0),\n\
         __actual__orders AS (SELECT * FROM __ref__orders),\n",
    )
}

pub(super) fn tsql_unprovable_cte_renames_are_refused_with_named_ctes() -> bool {
    let mut request = colliding_chain_request("tsql", "final");
    request["chain"][0]["comparisonBodySql"] = json!(
        "WITH final AS (SELECT 2 AS order_id FROM __ref__stg_orders) SELECT order_id AS final FROM final"
    );
    let error = render_one(request).expect_err("T-SQL refusal");
    assert_eq!(
        error,
        "CTE 'final' of model 'orders' collides with CTE 'final' of model 'stg_orders'; \
         T-SQL does not allow a nested WITH, so rename the CTE in the model or the fixture so \
         the names are unique"
    );
    true
}

pub(super) fn tsql_fixture_and_model_cte_collisions_are_renamed() -> bool {
    let sql = render_one(json!({
        "sqlAnalysisDialect": "tsql",
        "chain": [{
            "modelName": "orders",
            "resolvedSql": "WITH helper_rows AS (SELECT 1 AS order_id) SELECT order_id FROM helper_rows",
            "expectedCteSql": "SELECT order_id FROM helper_rows",
            "expectedLiftedCtes": [["helper_rows", "SELECT 2 AS order_id"]]
        }]
    }))
    .expect("T-SQL rename");
    sql.starts_with(
        "WITH helper_rows AS (SELECT 1 AS order_id),\n\
         __sqb_cte_0 AS (SELECT 2 AS order_id),\n\
         __actual__orders AS (SELECT order_id FROM helper_rows),\n\
         __expected__orders AS (SELECT order_id FROM __sqb_cte_0)\n",
    )
}

pub(super) fn tsql_identical_helper_ending_in_line_comment_is_shared() -> bool {
    let sql = render_one(json!({
        "sqlAnalysisDialect": "tsql",
        "chain": [{
            "modelName": "orders",
            "resolvedSql": "SELECT order_id FROM __source__raw",
            "liftedCtes": [["__source__raw", "WITH h AS (SELECT 1 AS order_id -- one\n) SELECT * FROM h"]],
            "comparisonBodySql": "SELECT order_id FROM __source__raw",
            "expectedCteSql": "SELECT order_id FROM h",
            "expectedLiftedCtes": [["h", "SELECT 1 AS order_id -- one"]]
        }]
    }))
    .expect("shared helper");
    sql.starts_with(
        "WITH h AS (SELECT 1 AS order_id -- one\n),\n__source__raw AS (SELECT * FROM h),\n",
    ) && sql.contains("__expected__orders AS (SELECT order_id FROM h)")
}

pub(super) fn trailing_statement_terminators_are_dropped() -> bool {
    [
        "WITH a AS (SELECT 1 AS id) SELECT * FROM a; -- done\n",
        "SELECT 1 AS id ;;\n",
    ]
    .iter()
    .all(|model| {
        let sql = render_one(json!({
            "sqlAnalysisDialect": "duckdb",
            "chain": [{"modelName": "m", "resolvedSql": model, "expectedCteSql": "SELECT 1 AS id"}]
        }))
        .expect("render");
        !sql.contains(';')
            && (sql.contains("__actual__m AS (SELECT * FROM a)")
                || sql.contains("__actual__m AS (SELECT 1 AS id)"))
    })
}

pub(super) fn tsql_distinct_ctes_lift_verbatim() -> bool {
    let sql = render_one(json!({
        "sqlAnalysisDialect": "tsql",
        "chain": [{
            "modelName": "orders",
            "resolvedSql": "WITH [base rows] (order_id) AS (SELECT 1 /* ) */ AS order_id), \
                            staged AS (SELECT order_id FROM [base rows]) SELECT TOP 5 order_id FROM staged",
            "expectedCteSql": "SELECT 1 AS order_id"
        }]
    }))
    .expect("distinct T-SQL CTEs lift");
    sql.starts_with(
        "WITH [base rows] (order_id) AS (SELECT 1 /* ) */ AS order_id),\n\
         staged AS (SELECT order_id FROM [base rows]),\n\
         __actual__orders AS (SELECT TOP 5 order_id FROM staged),\n",
    )
}

pub(super) fn snowflake_function_synonyms_stay_as_authored() -> bool {
    let model = "WITH picked AS (SELECT STARTSWITH(name, 'A') AS matches, SUBSTR(name, 1, 2) AS prefix, \
                 TIMESTAMPDIFF(day, created_at, updated_at) AS age, TRY_TO_DECIMAL(amount, 10, 2) AS total \
                 FROM items) SELECT * FROM picked";
    let sql = render_one(json!({
        "sqlAnalysisDialect": "snowflake",
        "chain": [{"modelName": "items", "resolvedSql": model, "expectedCteSql": "SELECT 1 AS matches"}]
    }))
    .expect("Snowflake render");
    let (ctes, body) = model.split_once(") SELECT").expect("authored model");
    sql.contains(&format!("{})", ctes.trim_start_matches("WITH ")))
        && sql.contains(&format!("__actual__items AS (SELECT{body})"))
        && !sql.contains("STARTS_WITH")
        && !sql.contains("SUBSTRING")
}

pub(crate) fn generic_lexical_syntax() -> Value {
    json!({
        "backslashEscapeQuotes": [],
        "escapeStringPrefix": false,
        "rawStringPrefix": false,
        "tripleQuotedStrings": false,
        "nestedBlockComments": false,
        "lineCommentPrefixes": ["--"]
    })
}

type MarkerCalls = Option<Vec<(String, String)>>;

/// The original JSON-tree walk, kept as the oracle the direct relation walk must match.
fn json_relation_marker_calls(value: &Value) -> Vec<(String, String)> {
    let from_markers = ["from", "from_clause"]
        .into_iter()
        .filter_map(|key| value.get(key))
        .filter_map(|from| from.get("expressions"))
        .filter_map(Value::as_array)
        .flatten()
        .chain(value.get("left").filter(|_| value.get("joins").is_some()))
        .filter_map(json_marker);
    let join_markers = ["joins", "from_joins"]
        .into_iter()
        .filter_map(|key| value.get(key))
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|join| join.get("this"))
        .filter_map(json_marker);
    let children: Vec<&Value> = value
        .as_object()
        .map(|object| object.values().collect())
        .or_else(|| value.as_array().map(|values| values.iter().collect()))
        .unwrap_or_default();
    from_markers
        .chain(join_markers)
        .chain(children.into_iter().flat_map(json_relation_marker_calls))
        .collect()
}

fn json_marker(expression: &Value) -> Option<(String, String)> {
    let object = expression.as_object()?;
    object.get("alias").map_or_else(
        || json_function_marker(object),
        |alias| alias.get("this").and_then(json_marker),
    )
}

fn json_function_marker(object: &serde_json::Map<String, Value>) -> Option<(String, String)> {
    let function = object.get("function")?.as_object()?;
    let function_name = function.get("name")?.as_str()?.to_ascii_lowercase();
    let names: Vec<String> = function
        .get("args")?
        .as_array()?
        .iter()
        .map(|arg| {
            arg.get("column")?
                .get("name")?
                .get("name")?
                .as_str()
                .map(str::to_string)
        })
        .collect::<Option<_>>()?;
    let referenced_name = (function_name == "__dbt_ref" && names.len() == 2)
        .then(|| names.join("__"))
        .or_else(|| (names.len() == 1).then(|| names[0].clone()))?;
    Some((function_name, referenced_name))
}

/// Owned marker calls for comparison with collected calls.
pub(crate) fn owned_marker_calls(calls: &[(&str, &str)]) -> MarkerCalls {
    Some(
        calls
            .iter()
            .map(|(function, name)| ((*function).to_string(), (*name).to_string()))
            .collect(),
    )
}

/// Parse one statement and return the direct walk's and the JSON oracle's marker calls.
pub(crate) fn relation_markers_and_json_oracle(
    dialect: &str,
    sql: &str,
) -> (MarkerCalls, MarkerCalls) {
    let mut statements = sql_test_parser(Some(dialect))
        .expect("test dialect must exist")
        .parse(sql)
        .expect("test SQL must parse")
        .statements;
    assert_eq!(statements.len(), 1, "{sql}");
    let expression = statements.remove(0);
    (
        Some(relation_marker_calls(&expression)),
        serde_json::to_value(&expression)
            .ok()
            .map(|value| json_relation_marker_calls(&value)),
    )
}

/// Return every generated relation shape whose collected markers differ from the JSON oracle.
pub(crate) fn relation_marker_oracle_mismatches(dialect: &str) -> Vec<String> {
    let relations: &[&str] = &[
        "__ref(\"orders\")",
        "__source(\"raw_orders\") AS s",
        "__dbt_ref(\"shop\", \"customers\") c",
        "(SELECT * FROM __seed(\"regions\") JOIN __ref(\"stores\") ON TRUE) AS d",
        "LATERAL (SELECT * FROM __ref(\"items\")) AS l",
        "__udf(\"fn_orders\")",
        "plain_table",
    ];
    let wrappers = [
        "SELECT * FROM {a} JOIN {b} ON TRUE",
        "WITH x AS (SELECT * FROM {a}) SELECT (SELECT 1 FROM {b}) AS v FROM x",
        "SELECT * FROM {a} WHERE EXISTS (SELECT 1 FROM {b})",
        "SELECT * FROM {a} UNION SELECT * FROM {b}",
        "SELECT * FROM {a}, {b}",
        "SELECT * FROM ({a} JOIN {b} ON TRUE)",
        "SELECT * FROM {a} AS x LEFT JOIN LATERAL (SELECT * FROM {b}) AS y ON TRUE",
        "SELECT * FROM (SELECT * FROM {a}) AS x CROSS JOIN {b} WHERE x.id IN (SELECT id FROM {a})",
        "UPDATE orders SET amount = 1 FROM {a} JOIN {b} ON TRUE",
        "DELETE FROM orders USING {a} JOIN {b} ON TRUE",
        "INSERT INTO orders SELECT * FROM {a} JOIN {b} ON TRUE",
    ];
    let parser = sql_test_parser(Some(dialect)).expect("test dialect must exist");
    wrappers
        .iter()
        .flat_map(|wrapper| {
            relations.iter().flat_map(move |first| {
                relations
                    .iter()
                    .map(move |second| wrapper.replace("{a}", first).replace("{b}", second))
            })
        })
        .filter(|sql| {
            parser
                .parse(sql)
                .map(|parsed| parsed.statements)
                .unwrap_or_default()
                .iter()
                .any(|expression| {
                    Some(relation_marker_calls(expression))
                        != serde_json::to_value(expression)
                            .ok()
                            .map(|value| json_relation_marker_calls(&value))
                })
        })
        .collect()
}

/// Dialects and their analysis modes; analysis reads BigQuery and Databricks markers as strings.
/// Only those with a native parser are planned; the others' SQL tests are unavailable.
const DIALECT_MODES: [(&str, &[bool]); 6] = [
    ("duckdb", &[true, false]),
    ("snowflake", &[true, false]),
    ("postgres", &[true, false]),
    ("bigquery", &[false]),
    ("databricks", &[false]),
    ("tsql", &[true, false]),
];

/// Plan a fixture-read helper beside a model on every dialect and check names stay isolated.
pub(crate) fn helper_names_are_isolated(shared: &SharedNameTestCase) -> bool {
    for (dialect, modes) in DIALECT_MODES {
        if sql_test_parser(Some(dialect)).is_none() {
            continue;
        }
        for &sql_analysis_enabled in modes {
            let label = format!(
                "{} on {dialect}, analysis {sql_analysis_enabled}",
                shared.description
            );
            let artifact = plan_shape(&PlanShape {
                dialect,
                sql_analysis_enabled,
                model_sql: shared.model_sql,
                helper_name: shared.helper_name,
                expected_sql: "SELECT 1 AS order_id",
            })
            .expect("plan");
            assert_eq!(artifact["warnings"], json!([]), "{label}");
            let sql = artifact["sql"].as_str().expect("rendered SQL");
            assert_names_isolated(sql, dialect, shared.helper_name, &label);
            assert_eq!(
                sql.matches(shared.expected_model_reads).count(),
                1,
                "{label}: {sql}"
            );
        }
    }
    true
}

/// The compile error, or else the first warning, of planning one shape.
pub(crate) fn plan_shape_refusal(shape: &PlanShape<'_>) -> String {
    plan_shape(shape)
        .map(|artifact| {
            artifact["warnings"][0]["message"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .unwrap_or_else(|error| error)
}

fn plan_shape(shape: &PlanShape<'_>) -> Result<Value, String> {
    let response = plan_json(
        &json!({
            "lexicalSyntax": generic_lexical_syntax(),
            "models": [{"name": "orders", "querySql": shape.model_sql, "modelDependencies": []}],
            "tests": [{"name": "orders_case", "fileLabel": "tests/orders.sql", "payload": {
                "kind": "model",
                "authoredCtes": [
                    {"name": shape.helper_name, "sqlBody": "SELECT 1 AS order_id"},
                    {"name": "__source__raw_orders", "sqlBody": format!("SELECT order_id FROM {}", shape.helper_name)}
                ],
                "expectedCtes": [{"name": "__expected__orders", "sqlBody": shape.expected_sql}],
                "expectedModelNames": ["orders"],
                "assertionCtes": []
            }}],
            "sqlAnalysisEnabled": shape.sql_analysis_enabled,
            "sqlAnalysisDialect": shape.dialect
        })
        .to_string(),
    )?;
    Ok(serde_json::from_str::<Value>(&response).expect("plan JSON")["artifacts"][0].clone())
}

/// Top-level CTE names and every nested CTE name are disjoint, and no helper keeps its name.
fn assert_names_isolated(sql: &str, dialect: &str, helper_name: &str, label: &str) {
    let slice_dialect = SliceDialect::new(Some(dialect));
    let split = split_top_level_with(sql, slice_dialect)
        .expect("rendered SQL scans")
        .expect("rendered SQL has a WITH");
    let top_level: HashSet<String> = split.ctes.iter().map(|cte| cte.key.clone()).collect();
    let helper_at_top_level = split.ctes.iter().any(|cte| {
        cte.key == helper_name
            && (!slice_dialect.rejects_nested_with() || cte.body.contains("SELECT 1 AS order_id"))
    });
    assert!(!helper_at_top_level, "{label}: {sql}");
    for cte in &split.ctes {
        let nested = defined_cte_keys(cte.body, slice_dialect);
        assert!(
            nested.iter().all(|name| !top_level.contains(name)),
            "{label}: {} nests {nested:?}: {sql}",
            cte.header
        );
    }
}

fn helper_reference_test(case: &HelperReferenceTestCase, sends_compiler_reads: bool) -> Value {
    let named = |ctes: &[(&str, &str)]| -> Vec<Value> {
        ctes.iter()
            .map(|(name, sql)| json!({"name": name, "sqlBody": sql}))
            .collect()
    };
    let mut authored = named(&[
        (
            "__source__raw_orders",
            "SELECT 1 AS order_id, 10 AS amount, 7 AS region_id",
        ),
        (
            "__seed__regions",
            "SELECT 7 AS region_id, 'north' AS region_name",
        ),
    ]);
    authored.extend(named(case.helpers));
    let mut test = json!({
        "name": "orders_case",
        "fileLabel": "tests/orders.sql",
        "payload": {
            "kind": "model",
            "authoredCtes": authored,
            "expectedCtes": named(case.expected),
            "expectedModelNames": case
                .expected
                .iter()
                .map(|(name, _)| name.trim_start_matches("__expected__"))
                .collect::<Vec<_>>(),
            "assertionCtes": named(case.assertions)
        }
    });
    let reads = [
        ("readHelperNames", json!(case.read_helpers)),
        ("referenceTargetModelNames", json!(case.reference_targets)),
    ];
    for (key, value) in reads.into_iter().filter(|_| sends_compiler_reads) {
        test["payload"][key] = value;
    }
    test
}

fn helper_reference_models() -> Value {
    json!([
        {
            "name": "stg_orders",
            "querySql": "SELECT order_id, amount, region_id FROM __source(\"raw_orders\")",
            "modelDependencies": []
        },
        {
            "name": "orders",
            "querySql": "SELECT order_id, amount * 2 AS amount_doubled FROM __ref(\"stg_orders\")",
            "modelDependencies": ["stg_orders"]
        }
    ])
}

/// Plan and render one helper-reference case through the native planner's JSON entry point.
pub(crate) fn plan_helper_reference_response(
    case: &HelperReferenceTestCase,
    sends_compiler_reads: bool,
) -> Result<String, String> {
    crate::compiler::_helpers::sql_tests::planning::plan_and_render_json(
        &json!({
            "lexicalSyntax": generic_lexical_syntax(),
            "models": helper_reference_models(),
            "tests": [helper_reference_test(case, sends_compiler_reads)],
            "sqlAnalysisEnabled": case.sql_analysis_enabled,
            "sqlAnalysisDialect": "duckdb",
            "setDifferenceOperator": "EXCEPT"
        })
        .to_string(),
    )
}

/// Plan and render one helper-reference case, returning its SQL, chain and warnings.
pub(crate) fn plan_helper_reference_case(case: &HelperReferenceTestCase) -> Value {
    let response: Value = serde_json::from_str(
        &plan_helper_reference_response(case, true).expect("test assumption must hold"),
    )
    .expect("test assumption must hold");
    response["artifacts"][0].clone()
}

/// Resolve one helper-reference case's model chain without planning SQL.
pub(crate) fn chain_helper_reference_case(case: &HelperReferenceTestCase) -> Value {
    let response: Value = serde_json::from_str(
        &crate::compiler::_helpers::sql_tests::planning::resolve_chains_json(
            &json!({
                "lexicalSyntax": generic_lexical_syntax(),
                "models": helper_reference_models(),
                "tests": [helper_reference_test(case, true)]
            })
            .to_string(),
        )
        .expect("test assumption must hold"),
    )
    .expect("test assumption must hold");
    response["chains"][0].clone()
}

/// Whether `first` is defined before `second` at the top level of a rendered test query.
pub(crate) fn defined_before(sql: &str, first: &str, second: &str) -> bool {
    top_level_cte_position(sql, first) < top_level_cte_position(sql, second)
}

/// Plan a request whose model tests predate the compiler's reads, sending empty reads for them.
pub(crate) fn plan_json(request_json: &str) -> Result<String, String> {
    crate::compiler::_helpers::sql_tests::planning::plan_and_render_json(
        &with_empty_compiler_reads(request_json),
    )
}

/// Resolve chains for a request whose model tests predate the compiler's reads.
pub(crate) fn chain_json(request_json: &str) -> Result<String, String> {
    crate::compiler::_helpers::sql_tests::planning::resolve_chains_json(&with_empty_compiler_reads(
        request_json,
    ))
}

fn with_empty_compiler_reads(request_json: &str) -> String {
    let mut request: Value = serde_json::from_str(request_json).expect("test assumption must hold");
    let model_payloads = request["tests"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .map(|test| &mut test["payload"])
        .filter(|payload| payload["kind"] == json!("model"))
        .filter_map(Value::as_object_mut);
    for payload in model_payloads {
        for key in ["readHelperNames", "referenceTargetModelNames"] {
            payload.entry(key).or_insert_with(|| json!([]));
        }
    }
    request.to_string()
}

/// The lexical rules of each adapter family the rule cases use, as the Python adapters declare them.
const LEXICAL_SYNTAXES: &str = r##"{"bigquery": {"backslashEscapeQuotes": ["\"", "'", "`"], "escapeStringPrefix": false, "rawStringPrefix": true, "tripleQuotedStrings": true, "nestedBlockComments": false, "lineCommentPrefixes": ["#", "--"]}, "duckdb": {"backslashEscapeQuotes": [], "escapeStringPrefix": true, "rawStringPrefix": false, "tripleQuotedStrings": false, "nestedBlockComments": true, "lineCommentPrefixes": ["--"]}, "snowflake": {"backslashEscapeQuotes": ["'"], "escapeStringPrefix": false, "rawStringPrefix": false, "tripleQuotedStrings": false, "nestedBlockComments": false, "lineCommentPrefixes": ["--", "//"]}, "generic": {"backslashEscapeQuotes": [], "escapeStringPrefix": false, "rawStringPrefix": false, "tripleQuotedStrings": false, "nestedBlockComments": false, "lineCommentPrefixes": ["--"]}}"##;

/// Extract one rule case and describe the outcome as `accepted_outcome` or `rejected_outcome` do.
pub(crate) fn extraction_rule_outcome(test_case: &ExtractionRuleTestCase) -> String {
    let syntaxes: Value = serde_json::from_str(LEXICAL_SYNTAXES).expect("the syntaxes are JSON");
    let response = batch_response(&json!({
        "syntax": syntaxes[test_case.syntax],
        "tests": [{"sql": test_case.sql, "fileLabel": "tests/t.sql", "mode": test_case.mode, "raw": test_case.raw}],
    }));
    let invalid_calls = response["tests"][0]["invalidCalls"]
        .as_bool()
        .unwrap_or(false);
    response.get("error").map_or_else(
        || accepted_outcome(invalid_calls),
        |error| {
            let offset = error["tokenOffset"]
                .as_u64()
                .and_then(|offset| usize::try_from(offset).ok());
            rejected_outcome(
                error["message"].as_str().unwrap_or_default(),
                error["help"].as_str(),
                error["token"].as_str().zip(offset),
            )
        },
    )
}

fn batch_response(request: &Value) -> Value {
    serde_json::from_str(&extract_batch_json(&request.to_string()).expect("the request is valid"))
        .expect("the response is JSON")
}

/// An accepted test; `invalid_calls` when a helper or expected CTE has a malformed reference call.
pub(crate) fn accepted_outcome(invalid_calls: bool) -> String {
    format!("accepted: invalid calls {invalid_calls}")
}

/// A rejected test's first error message, its help, and its offending text with its offset.
pub(crate) fn rejected_outcome(
    message: &str,
    help: Option<&str>,
    token: Option<(&str, usize)>,
) -> String {
    format!(
        "rejected: {message} | help: {} | token: {}",
        help.map_or(Value::Null, |help| Value::String(help.to_owned())),
        token.map_or(json!([null, null]), |(text, offset)| json!([text, offset])),
    )
}

/// The authored-block reading of `sql`: its CTEs with body offsets, or its first error.
pub(crate) fn authored_ctes_response(sql: &str, scenario: bool) -> Value {
    batch_response(
        &json!({"tests": [{"sql": sql, "fileLabel": "tests/t.sql", "mode": "model", "authored": true, "scenario": scenario}]}),
    )
}
