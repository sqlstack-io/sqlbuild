use serde_json::{Value, json};

use crate::compiler::_helpers::sql_tests::planning::plan_and_render_json;
use crate::compiler::_helpers::sql_tests::planning::resolve_chains_json;
use crate::compiler::_helpers::sql_tests::rendering::sql_test_parser;
use crate::compiler::main::sql_test_chains::resolve_sql_test_chains;
use crate::compiler::main::sql_test_glue::plan_sql_test_batch;
use crate::compiler::main::sql_test_plan_errors::sql_test_plan_error_messages;
use crate::compiler::models::{SqlTestPlan, SqlTestPlanWarning};
use crate::compiler::tests::helpers::{
    chain_resolution_orders_unmocked_models, concurrent_requests_initialize_shared_template_once,
    deep_shared_graph_reports_missing_mock_once,
    helper_ctes_are_in_scope_for_assertions_and_expected_rows, long_chain_plan_output_stays_linear,
    mock_read_through_helper_brings_its_mock_dependencies_into_scope,
    model_test_batch_returns_ordered_artifact, plan_without_rendering_returns_executable_steps,
    scoped_helper_named_like_generated_cte_is_rejected,
    shared_textual_chain_renders_each_model_once, textual_assertion_with_clause_merges_lifted_ctes,
    unicode_cte_after_leading_with_preserves_identifier,
    unresolved_reference_fast_rejection_preserves_warning, upstream_fallback_resolves,
};
use crate::compiler::tests::planning_helpers::projected_udf_call_sql;
use crate::compiler::tests::test_types::{
    GlueErrorProjectionTestCase, GluePlanParityTestCase, GluePlannerErrorTestCase,
    ProjectedUdfTestCase, SqlTestPlanningTestCase,
};

/// A mocked model test, a test missing its source mock and a UDF test over two models.
const GLUE_REQUEST: &str = r#"{
  "sqlAnalysisDialect": "duckdb",
  "lexicalSyntax": {
    "backslashEscapeQuotes": [], "escapeStringPrefix": false, "rawStringPrefix": false,
    "tripleQuotedStrings": false, "nestedBlockComments": false, "lineCommentPrefixes": ["--"]
  },
  "models": [
    {"name": "stg_orders", "querySql": "SELECT order_id, amount FROM __source(\"raw_orders\")"},
    {
      "name": "orders",
      "querySql": "WITH big AS (SELECT * FROM __ref(\"stg_orders\") WHERE __udf(\"udf__is_big\")(amount)) SELECT order_id FROM big",
      "modelDependencies": ["stg_orders"]
    }
  ],
  "functions": [{
    "name": "udf__is_big", "udfPrefix": "main.udf__is_big(", "udfSuffix": ")",
    "tableFunctionPrefix": "main.udf__is_big(", "tableFunctionSuffix": ")"
  }],
  "tests": [
    {
      "name": "mocked", "fileLabel": "tests/mocked.sql",
      "payload": {
        "kind": "model",
        "authoredCtes": [
          {"name": "__source__raw_orders", "sqlBody": "SELECT 1 AS order_id, 5 AS amount"},
          {"name": "__seed__unused", "sqlBody": "SELECT 1 AS id"}
        ],
        "expectedCtes": [{"name": "__expected__orders", "sqlBody": "SELECT 1 AS order_id"}],
        "expectedModelNames": ["orders"],
        "assertionCtes": [{
          "name": "__assert__orders_exist",
          "sqlBody": "SELECT * FROM __ref(\"orders\") WHERE 1 = 0"
        }],
        "readHelperNames": [], "referenceTargetModelNames": []
      }
    },
    {
      "name": "missing_mock", "fileLabel": "tests/missing_mock.sql",
      "payload": {
        "kind": "model",
        "authoredCtes": [{"name": "__seed__unused", "sqlBody": "SELECT 1 AS id"}],
        "expectedCtes": [{"name": "__expected__orders", "sqlBody": "SELECT 1 AS order_id"}],
        "expectedModelNames": ["orders"],
        "readHelperNames": [], "referenceTargetModelNames": []
      }
    },
    {
      "name": "udf_case", "fileLabel": "tests/udf_case.sql",
      "payload": {
        "kind": "direct", "mode": "udf",
        "actualCte": {
          "name": "__udf_actual__", "sqlBody": "SELECT __udf(\"udf__is_big\")(150) AS is_big"
        },
        "expectedCte": {"name": "__udf_expected__", "sqlBody": "SELECT TRUE AS is_big"}
      }
    }
  ]
}"#;

#[test]
fn given_sql_test_upstream_fallback_when_planning_then_descendants_and_assertions_resolve() {
    let test_cases = [SqlTestPlanningTestCase {
        description: "textual upstream steps remain available to descendants and assertions",
        run: upstream_fallback_resolves,
        expected_success: true,
    }];
    for test_case in test_cases {
        assert_eq!(
            (test_case.run)(),
            test_case.expected_success,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sql_test_planning_cases_when_exercising_native_planner_then_expected_behavior_holds() {
    let test_cases = [
        SqlTestPlanningTestCase {
            description: "concurrent requests initialize the shared template once",
            run: concurrent_requests_initialize_shared_template_once,
            expected_success: true,
        },
        SqlTestPlanningTestCase {
            description: "model test batches return ordered artifacts",
            run: model_test_batch_returns_ordered_artifact,
            expected_success: true,
        },
        SqlTestPlanningTestCase {
            description: "Unicode CTE identifiers survive leading WITH extraction",
            run: unicode_cte_after_leading_with_preserves_identifier,
            expected_success: true,
        },
        SqlTestPlanningTestCase {
            description: "shared textual chain renders each upstream model once",
            run: shared_textual_chain_renders_each_model_once,
            expected_success: true,
        },
        SqlTestPlanningTestCase {
            description: "fast rejection preserves unresolved reference warnings",
            run: unresolved_reference_fast_rejection_preserves_warning,
            expected_success: true,
        },
        SqlTestPlanningTestCase {
            description: "deep shared graph reports one missing mock once",
            run: deep_shared_graph_reports_missing_mock_once,
            expected_success: true,
        },
        SqlTestPlanningTestCase {
            description: "planning without rendering returns executable steps",
            run: plan_without_rendering_returns_executable_steps,
            expected_success: true,
        },
        SqlTestPlanningTestCase {
            description: "long chain plan output stays linear",
            run: long_chain_plan_output_stays_linear,
            expected_success: true,
        },
        SqlTestPlanningTestCase {
            description: "chain resolution orders unmocked models",
            run: chain_resolution_orders_unmocked_models,
            expected_success: true,
        },
        SqlTestPlanningTestCase {
            description: "helper CTEs are in scope for assertions and expected rows",
            run: helper_ctes_are_in_scope_for_assertions_and_expected_rows,
            expected_success: true,
        },
        SqlTestPlanningTestCase {
            description: "mock read through a helper brings its mock dependencies into scope",
            run: mock_read_through_helper_brings_its_mock_dependencies_into_scope,
            expected_success: true,
        },
        SqlTestPlanningTestCase {
            description: "scoped helper named like a generated CTE is rejected",
            run: scoped_helper_named_like_generated_cte_is_rejected,
            expected_success: true,
        },
        SqlTestPlanningTestCase {
            description: "textual assertion WITH clause merges lifted CTEs",
            run: textual_assertion_with_clause_merges_lifted_ctes,
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
fn given_typed_requests_when_planning_then_plans_and_chains_equal_the_json_entry_points() {
    let test_cases = [
        GluePlanParityTestCase {
            description: "DuckDB, planned without rendering",
            dialect: Some("duckdb"),
            render_sql: false,
            expected_plan_count: 3,
            expected_missing_mock_severity: "error",
        },
        GluePlanParityTestCase {
            description: "DuckDB, rendered",
            dialect: Some("duckdb"),
            render_sql: true,
            expected_plan_count: 3,
            expected_missing_mock_severity: "error",
        },
        GluePlanParityTestCase {
            description: "Snowflake, rendered",
            dialect: Some("snowflake"),
            render_sql: true,
            expected_plan_count: 3,
            expected_missing_mock_severity: "error",
        },
        GluePlanParityTestCase {
            description: "SQL Server rejects nested WITH and renders anyway",
            dialect: Some("tsql"),
            render_sql: true,
            expected_plan_count: 3,
            expected_missing_mock_severity: "error",
        },
    ];
    let without_timings = |mut value: Value| {
        for key in ["planningNs", "renderingNs"] {
            let _ = value.as_object_mut().map(|object| object.remove(key));
        }
        value
    };
    for test_case in test_cases
        .into_iter()
        .filter(|test_case| sql_test_parser(test_case.dialect).is_some())
    {
        let mut request: Value = serde_json::from_str(GLUE_REQUEST).expect("valid request");
        request["sqlAnalysisDialect"] = json!(test_case.dialect);
        request["renderSql"] = json!(test_case.render_sql);
        request["includePlan"] = json!(!test_case.render_sql);
        let typed = plan_sql_test_batch(serde_json::from_value(request.clone()).expect("typed"))
            .expect("typed plans");
        let typed: Value = without_timings(serde_json::to_value(&typed).expect("serializable"));
        let json: Value = without_timings(
            serde_json::from_str(&plan_and_render_json(&request.to_string()).expect("plans"))
                .expect("JSON plans"),
        );
        let typed_chains = resolve_sql_test_chains(
            serde_json::from_value(request.clone()).expect("typed chains request"),
        )
        .expect("typed chains");
        let json_chains: Value =
            serde_json::from_str(&resolve_chains_json(&request.to_string()).expect("chains"))
                .expect("JSON chains");
        assert_eq!(
            (
                typed["artifacts"].as_array().map(Vec::len),
                typed["artifacts"][1]["warnings"][0]["severity"].clone(),
                typed == json,
                json!({"chains": typed_chains}) == json_chains,
            ),
            (
                Some(test_case.expected_plan_count),
                json!(test_case.expected_missing_mock_severity),
                true,
                true,
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_test_without_compiler_reads_when_planning_then_the_typed_batch_returns_its_error() {
    let test_cases = [
        GluePlannerErrorTestCase {
            description: "helper reads are required for model tests",
            payload_field: "readHelperNames",
            expected_error_prefix: "planner_input:",
        },
        GluePlannerErrorTestCase {
            description: "reference targets are required for model tests",
            payload_field: "referenceTargetModelNames",
            expected_error_prefix: "planner_input:",
        },
    ];
    for test_case in test_cases {
        let mut request: Value = serde_json::from_str(GLUE_REQUEST).expect("valid request");
        request["tests"][0]["payload"][test_case.payload_field] = json!(null);
        let error = plan_sql_test_batch(serde_json::from_value(request).expect("typed")).err();
        assert_eq!(
            error.map(|message| message.starts_with(test_case.expected_error_prefix)),
            Some(true),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_plan_warnings_when_projecting_errors_then_each_distinct_error_message_is_kept_in_order() {
    let test_cases = [
        GlueErrorProjectionTestCase {
            description: "warnings are not errors",
            warnings: &[("warning", "mock is unreachable"), ("info", "note")],
            expected_messages: &[],
        },
        GlueErrorProjectionTestCase {
            description: "repeated errors are reported once, in first-seen order",
            warnings: &[
                ("error", "b has no mock"),
                ("warning", "a is unreachable"),
                ("error", "a has no mock"),
                ("error", "b has no mock"),
            ],
            expected_messages: &["b has no mock", "a has no mock"],
        },
    ];
    for test_case in test_cases {
        let plan = SqlTestPlan {
            sql: None,
            chain: Vec::new(),
            assertions: Vec::new(),
            model_names: Vec::new(),
            warnings: test_case
                .warnings
                .iter()
                .map(|(severity, message)| SqlTestPlanWarning {
                    model_name: None,
                    severity,
                    message: (*message).to_owned(),
                })
                .collect(),
        };
        assert_eq!(
            sql_test_plan_error_messages(&plan),
            test_case.expected_messages.to_vec(),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_real_model_calling_udf_in_projection_when_planning_then_call_targets_the_function() {
    let test_cases = [
        ProjectedUdfTestCase {
            description: "DuckDB falls back to textual resolution",
            dialect: "duckdb",
            expected_call: "analytics__is_open(o.status)",
        },
        ProjectedUdfTestCase {
            description: "ClickHouse parses the marker as a parametric call",
            dialect: "clickhouse",
            expected_call: "analytics__is_open(o.status)",
        },
    ];

    for test_case in test_cases
        .iter()
        .filter(|test_case| sql_test_parser(Some(test_case.dialect)).is_some())
    {
        let sql = projected_udf_call_sql(test_case.dialect);

        assert_eq!(
            (
                sql.contains(test_case.expected_call),
                sql.contains("__udf(")
            ),
            (true, false),
            "{}: {sql}",
            test_case.description
        );
    }
}
