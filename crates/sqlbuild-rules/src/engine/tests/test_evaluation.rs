use serde_json::{Value, json};
use tempfile::TempDir;

use crate::constants::NATIVE_BUILD_IDENTITY;
use crate::engine::_helpers::evaluation::evaluate_json;
use crate::engine::main::build_models::build_models;
use crate::engine::tests::helpers;
use crate::engine::tests::test_types;
use crate::models::{Model, ModelRow, ModelRowsContext};
use crate::query_facts::main::model_shape::model_shape;
use crate::query_facts::models::ModelSource;

#[test]
fn given_dependency_tokens_when_classifying_imports_then_preserves_dialect_and_original_sql()
-> Result<(), String> {
    let test_cases = [
        test_types::DependencyTokenTestCase {
            description: "comments and escaped literals retain canonical arguments",
            expected_columns: &[],
            dialect: "duckdb",
            query_sql: "WITH imported AS (SELECT * FROM __ReF /* __ref('ignored') */ ('Order''s')) SELECT '__source(''ignored'')' AS label FROM imported",
            expected_messages: &[],
        },
        test_types::DependencyTokenTestCase {
            description: "quoted function names are not dependency calls",
            expected_columns: &[],
            dialect: "duckdb",
            query_sql: "SELECT * FROM \"__ref\"('orders')",
            expected_messages: &[],
        },
        test_types::DependencyTokenTestCase {
            description: "backtick quoting and comment-separated source arguments",
            expected_columns: &[],
            dialect: "bigquery",
            query_sql: "WITH imported AS (SELECT * FROM __source /* ignored */ (`Orders`)) SELECT order_id FROM imported",
            expected_messages: &[],
        },
        test_types::DependencyTokenTestCase {
            description: "dollar strings do not contribute spurious calls",
            expected_columns: &[],
            dialect: "postgres",
            query_sql: "WITH imported AS (SELECT * FROM __ref('orders')) SELECT $$__ref('ignored')$$ AS label FROM imported",
            expected_messages: &[],
        },
        test_types::DependencyTokenTestCase {
            description: "generic fallback retains original dialect dependencies",
            expected_columns: &[],
            dialect: "postgres",
            query_sql: "WITH imported AS (SELECT * FROM __ref('orders')) SELECT payload:order_id FROM imported",
            expected_messages: &[],
        },
        test_types::DependencyTokenTestCase {
            description: "normalization must not erase dependency calls",
            expected_columns: &[1],
            dialect: "snowflake",
            query_sql: "SELECT * EXCLUDE (__ref('orders')) FROM orders",
            expected_messages: &[
                "each __ref/__source must appear once in its own top-level import CTE",
            ],
        },
        test_types::DependencyTokenTestCase {
            description: "a dependency passed to a table function is not an import CTE",
            expected_columns: &[1],
            dialect: "duckdb",
            query_sql: "WITH imported AS (SELECT * FROM __table_fn(\"unnested\")(__ref('orders'))) SELECT order_id FROM imported",
            expected_messages: &[
                "each __ref/__source must appear once in its own top-level import CTE",
            ],
        },
        test_types::DependencyTokenTestCase {
            description: "duplicate dependencies remain ordered findings",
            expected_columns: &[1, 55],
            dialect: "duckdb",
            query_sql: "WITH first_import AS (SELECT * FROM __ref('orders')), second_import AS (SELECT * FROM __REF('orders') WHERE order_id > 0) SELECT order_id FROM first_import",
            expected_messages: &[
                "each __ref/__source must appear once in its own top-level import CTE",
                "import CTE \"second_import\" contains transformation logic",
            ],
        },
    ];
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let config = json!({"select": ["SQBRMODEL101"], "cache": {"enabled": false}});
    for test_case in test_cases {
        let mut request: Value = serde_json::from_str(&helpers::request(&project_dir, &config))
            .map_err(|error| error.to_string())?;
        request["dialect"] = json!(test_case.dialect);
        request["models"][0]["query_sql"] = json!(test_case.query_sql);
        // Authored SQL can differ after expansion: dependency classification has
        // always used query_sql, while literal decisions use authored_sql.
        request["models"][0]["authored_sql"] = json!("SELECT @py.orders()");
        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        let faults = result["faults"].as_array().ok_or("missing faults")?;
        let messages: Vec<&str> = faults
            .iter()
            .filter_map(|fault| fault["message"].as_str())
            .collect();
        assert_eq!(
            messages, test_case.expected_messages,
            "{}: {result}",
            test_case.description
        );
        assert!(
            faults.iter().all(|fault| fault["unevaluated"] != true),
            "{}: {result}",
            test_case.description
        );
        for (fault, expected_column) in faults.iter().zip(test_case.expected_columns) {
            assert_eq!(fault["line"], 1, "{}", test_case.description);
            assert_eq!(
                fault["column"], *expected_column,
                "{}",
                test_case.description
            );
        }
    }
    Ok(())
}

#[test]
fn given_conflicting_model_identity_when_evaluating_layer_rules_then_reports_independent_findings()
-> Result<(), String> {
    let test_cases = [test_types::ModelLayerRulesTestCase {
        description: "conflicting model metadata reports every independent boundary",
        expected_codes: &[
            "SQBRMODEL103",
            "SQBRMODEL104",
            "SQBRPROJECT101",
            "SQBRPROJECT104",
            "SQBRPROJECT105",
            "SQBRPROJECT106",
        ],
    }];
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let config = json!({
        "select": [
            "SQBRMODEL103",
            "SQBRMODEL104",
            "SQBRPROJECT101",
            "SQBRPROJECT104",
            "SQBRPROJECT105",
            "SQBRPROJECT106"
        ],
        "cache": {"enabled": false}
    });
    let mut request: Value = serde_json::from_str(&helpers::request(&project_dir, &config))
        .map_err(|error| error.to_string())?;
    request["models"][0] = json!({
        "name": "commerce__mart_v__int_v_orders",
        "relative_path": "models/commerce/intermediate/enriched/orders/commerce__mart_v__int_v_orders.sql",
        "query_sql": "SELECT 1 AS order_id",
        "authored_sql": "SELECT 1 AS order_id",
        "config": {"materialized": "table", "schema": "staging"},
        "authored_config_keys": ["materialized", "schema"],
        "logical_schema": "staging",
        "references": [{"ref_kind": "ref", "ref_name": "commerce__mart__stg_orders"}]
    });

    for test_case in test_cases {
        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        let codes: Vec<&str> = result["faults"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|fault| fault["code"].as_str())
            .collect();

        assert_eq!(codes, test_case.expected_codes, "{}", test_case.description);
    }
    Ok(())
}

#[test]
fn given_internal_dependency_edges_when_evaluating_graph_rule_then_only_exact_live_exceptions_apply()
-> Result<(), String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let models = json!([
        {
            "name": "commerce__stg__orders",
            "relative_path": "models/commerce/staging/commerce__stg__orders.sql",
            "query_sql": "SELECT 1",
            "authored_sql": "SELECT 1",
            "references": [
                {"ref_kind": "ref", "ref_name": "commerce__int_enriched__orders"},
                {"ref_kind": "ref", "ref_name": "commerce__mart__customers"}
            ]
        },
        {
            "name": "commerce__int_clean__orders",
            "relative_path": "models/commerce/intermediate/clean/commerce__int_clean__orders.sql",
            "query_sql": "SELECT 1",
            "authored_sql": "SELECT 1",
            "references": [{"ref_kind": "ref", "ref_name": "commerce__int_enriched__customers"}]
        },
        {
            "name": "commerce__mart__summary",
            "relative_path": "models/commerce/mart/commerce__mart__summary.sql",
            "query_sql": "SELECT 1",
            "authored_sql": "SELECT 1",
            "references": [{"ref_kind": "ref", "ref_name": "commerce__int_enriched__orders"}]
        }
    ]);
    let test_cases = [
        test_types::GraphRuleTestCase {
            description: "unexcluded internal inversions",
            exceptions: json!([]),
            expected_fault_count: 2,
            expected_message_fragments: &[],
        },
        test_types::GraphRuleTestCase {
            description: "exact live exceptions",
            exceptions: json!([
                {
                    "consumer": "commerce__stg__orders",
                    "dependency": "commerce__int_enriched__orders",
                    "reason": "Tracked cleanup"
                },
                {
                    "consumer": "commerce__int_clean__orders",
                    "dependency": "commerce__int_enriched__customers",
                    "reason": "Tracked cleanup"
                }
            ]),
            expected_fault_count: 0,
            expected_message_fragments: &[],
        },
        test_types::GraphRuleTestCase {
            description: "stale exact exception",
            exceptions: json!([{
                "consumer": "commerce__stg__orders",
                "dependency": "commerce__int_clean__missing",
                "reason": "Resolved cleanup"
            }]),
            expected_fault_count: 3,
            expected_message_fragments: &["graph edge exception is stale"],
        },
    ];
    for test_case in test_cases {
        let config = json!({
            "select": ["SQBRGRAPH101"],
            "graph_edge_exceptions": test_case.exceptions,
            "cache": {"enabled": false}
        });
        let mut request: Value = serde_json::from_str(&helpers::request(&project_dir, &config))
            .map_err(|error| error.to_string())?;
        request["models"] = models.clone();
        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        let faults = result["faults"]
            .as_array()
            .ok_or("faults must be an array")?;
        assert_eq!(
            faults.len(),
            test_case.expected_fault_count,
            "{}",
            test_case.description
        );
        for fragment in test_case.expected_message_fragments {
            assert!(
                faults.iter().any(|fault| fault["message"]
                    .as_str()
                    .is_some_and(|message| message.contains(fragment))),
                "{}",
                test_case.description
            );
        }
    }
    Ok(())
}

#[test]
fn given_repeated_native_evaluation_when_faulting_then_returns_deterministic_complete_facts()
-> Result<(), String> {
    let test_cases = [test_types::NativeEvaluationTestCase {
        description: "repeated native evaluation preserves every fault fact",
        config: json!({
            "select": ["SQBRCONTRACT101"],
            "cache": {"enabled": false}
        }),
        expected_faults: json!([{
            "code": "SQBRCONTRACT101",
            "path": "models/mart/commerce__mart__orders.sql",
            "line": 1,
            "column": 1,
            "message": "models must declare an enforced output contract",
            "remediation": "Declare contract enforced and list the authoritative output columns in MODEL()."
        }]),
    }];

    for test_case in &test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let request_json = helpers::request(&project_dir, &test_case.config);
        let first: Value = serde_json::from_str(&evaluate_json(&request_json)?)
            .map_err(|error| error.to_string())?;
        let second: Value = serde_json::from_str(&evaluate_json(&request_json)?)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            first["faults"], second["faults"],
            "{}",
            test_case.description
        );
        assert_eq!(
            first["faults"], test_case.expected_faults,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_enforced_contract_when_column_type_is_missing_then_typed_contract_rule_faults()
-> Result<(), String> {
    let test_cases = [
        test_types::TypedContractColumnTestCase {
            description: "enforced contract has one untyped column",
            contract: "enforced",
            columns: json!([
                {"name": "order_id", "type": "INTEGER", "type_proven": true},
                {"name": "status", "type": "", "type_proven": false}
            ]),
            expected_fault_count: 1,
            expected_messages: &["contract column \"status\" has no declared type"],
        },
        test_types::TypedContractColumnTestCase {
            description: "enforced contract has only typed columns",
            contract: "enforced",
            columns: json!([
                {"name": "order_id", "type": "INTEGER", "type_proven": true}
            ]),
            expected_fault_count: 0,
            expected_messages: &[],
        },
        test_types::TypedContractColumnTestCase {
            description: "unenforced column metadata remains outside this rule",
            contract: "none",
            columns: json!([
                {"name": "status", "type": "", "type_proven": false}
            ]),
            expected_fault_count: 0,
            expected_messages: &[],
        },
    ];

    for test_case in test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let mut request: Value = serde_json::from_str(&helpers::request(
            &project_dir,
            &json!({
                "select": ["SQBRCONTRACT106"],
                "cache": {"enabled": false}
            }),
        ))
        .map_err(|error| error.to_string())?;
        request["models"][0]["config"] = json!({"contract": test_case.contract});
        request["models"][0]["columns"] = test_case.columns;

        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        let faults = result["faults"]
            .as_array()
            .ok_or_else(|| "faults must be an array".to_string())?;
        let messages = faults
            .iter()
            .filter_map(|fault| fault["message"].as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            faults.len(),
            test_case.expected_fault_count,
            "{}",
            test_case.description
        );
        assert_eq!(
            messages, test_case.expected_messages,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_scoped_suppression_when_evaluating_native_fault_then_returns_no_faults()
-> Result<(), String> {
    let test_cases = [test_types::NativeEvaluationTestCase {
        description: "native evaluation applies a matching scoped suppression",
        config: json!({
            "select": ["SQBRCONTRACT101"],
            "cache": {"enabled": false},
            "rule_ignores": [{
                "rules": ["SQBRCONTRACT"],
                "paths": ["models/mart/**"],
                "reason": "Tracked migration"
            }]
        }),
        expected_faults: json!([]),
    }];

    for test_case in &test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let request_json = helpers::request(&project_dir, &test_case.config);
        let result: Value = serde_json::from_str(&evaluate_json(&request_json)?)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            result["faults"], test_case.expected_faults,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_declaration_only_project_when_evaluating_project_rule_then_rule_still_runs()
-> Result<(), String> {
    let test_cases = [test_types::NativeEvaluationTestCase {
        description: "duplicate enum rule runs without a model anchor",
        config: json!({
            "select": ["SQBRDECLARATION201"],
            "cache": {"enabled": false}
        }),
        expected_faults: json!(["SQBRDECLARATION201"]),
    }];
    for test_case in test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let mut request: Value =
            serde_json::from_str(&helpers::request(&project_dir, &test_case.config))
                .map_err(|error| error.to_string())?;
        request["models"] = json!([]);
        request["public_enums"] = json!([
            {
                "name": "order_status",
                "relative_path": "enums/orders/order_status.sql",
                "members": [{"name": "OPEN", "value": "open"}]
            },
            {
                "name": "support_status",
                "relative_path": "enums/support/support_status.sql",
                "members": [{"name": "OPEN", "value": "open"}]
            }
        ]);
        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        let actual_codes = Value::Array(
            result["faults"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|fault| fault["code"].clone())
                .collect(),
        );
        assert_eq!(
            actual_codes, test_case.expected_faults,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_scoped_and_global_declarations_when_evaluating_domain_rule_then_only_global_is_checked()
-> Result<(), String> {
    let test_cases = [
        test_types::DeclarationScopeTestCase {
            description: "local test declaration is outside global domain policy",
            scope: "local",
            path: "tests/unit/orders/_sqlbuild/enums/status.sql",
            expected_fault_count: 0,
        },
        test_types::DeclarationScopeTestCase {
            description: "global declaration without domain remains in policy",
            scope: "global",
            path: "enums/status.sql",
            expected_fault_count: 1,
        },
    ];
    for test_case in test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let config = json!({
            "select": ["SQBRDECLARATION301"],
            "domains": ["orders"],
            "cache": {"enabled": false}
        });
        let mut request: Value = serde_json::from_str(&helpers::request(&project_dir, &config))
            .map_err(|error| error.to_string())?;
        request["models"] = json!([]);
        request["scope_index"] = helpers::scope_index();
        request["scope_index"]["declarations"][0]["scope"] = json!(test_case.scope);
        request["scope_index"]["declarations"][0]["path"] = json!(test_case.path);
        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        assert_eq!(
            result["faults"].as_array().map_or(0, Vec::len),
            test_case.expected_fault_count,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_snowflake_expression_when_evaluating_rules_then_uses_project_dialect() -> Result<(), String>
{
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let test_cases = [
        test_types::DialectEvaluationTestCase {
            description: "Snowflake compiler expressions use the resolved project dialect",
            dialect: "snowflake",
            query_sql: concat!(
                "SELECT * EXCLUDE (ignored), ",
                "TRANSFORM(values, leg INT -> leg + 1) AS adjusted, ",
                "payload:product_id::STRING AS product_id, ",
                "CAST(amounts AS ARRAY(NUMBER(38, 10))) AS amounts, ",
                "OBJECT_CONSTRUCT_KEEP_NULL('group_id', group_id) AS details, ",
                "FROM __table_fn(\"inventory_items\")(42)"
            ),
            expected_code: "SQBRCONTRACT101",
        },
        test_types::DialectEvaluationTestCase {
            description: "a Snowflake table function in an aliased CTE source parses as SQLBuild renders it",
            dialect: "snowflake",
            query_sql: concat!(
                "WITH lines AS (SELECT * FROM __table_fn(\"lines_for\")('B-1', __ref(\"orders\").k) AS l)\n",
                "SELECT l.item_id FROM lines AS l"
            ),
            expected_code: "SQBRCONTRACT101",
        },
        test_types::DialectEvaluationTestCase {
            description: "a Snowflake table function already written inside TABLE( ) is not wrapped twice",
            dialect: "snowflake",
            query_sql: "SELECT item_id FROM table ( __table_fn(\"inventory_items\")(42) )",
            expected_code: "SQBRCONTRACT101",
        },
    ];

    for test_case in test_cases {
        let config = json!({"select": ["SQBRCONTRACT101"], "cache": {"enabled": false}});
        let mut request: Value = serde_json::from_str(&helpers::request(&project_dir, &config))
            .map_err(|error| error.to_string())?;
        request["dialect"] = json!(test_case.dialect);
        request["models"][0]["query_sql"] = json!(test_case.query_sql);
        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        assert_eq!(
            result["faults"][0]["code"], test_case.expected_code,
            "{}",
            test_case.description
        );
        assert_ne!(
            result["faults"][0]["unevaluated"], true,
            "{}: {result}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_dialect_quoting_around_table_functions_when_evaluating_rules_then_model_parses()
-> Result<(), String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let test_cases = [
        test_types::DialectEvaluationTestCase {
            description: "Snowflake backslash-escaped quote before a table function",
            dialect: "snowflake",
            query_sql: concat!(
                "SELECT 'it\\'s (' AS label, item_id ",
                "FROM __table_fn(\"inventory_items\")(42)"
            ),
            expected_code: "SQBRCONTRACT101",
        },
        test_types::DialectEvaluationTestCase {
            description: "BigQuery backslash-escaped quote inside table function arguments",
            dialect: "bigquery",
            query_sql: concat!(
                "SELECT item_id ",
                "FROM __table_fn(\"inventory_items\", 'a\\')')(42)"
            ),
            expected_code: "SQBRCONTRACT101",
        },
        test_types::DialectEvaluationTestCase {
            description: "BigQuery backtick identifier containing a parenthesis in arguments",
            dialect: "bigquery",
            query_sql: concat!(
                "SELECT item_id ",
                "FROM __table_fn(\"inventory_items\", `batch)size`)(42)"
            ),
            expected_code: "SQBRCONTRACT101",
        },
    ];

    for test_case in test_cases {
        let config = json!({"select": ["SQBRCONTRACT101"], "cache": {"enabled": false}});
        let mut request: Value = serde_json::from_str(&helpers::request(&project_dir, &config))
            .map_err(|error| error.to_string())?;
        request["dialect"] = json!(test_case.dialect);
        request["models"][0]["query_sql"] = json!(test_case.query_sql);
        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        assert_eq!(
            result["faults"][0]["code"], test_case.expected_code,
            "{}: {result}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_unparseable_model_when_evaluating_then_retains_named_rule_fault() -> Result<(), String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let test_cases = [test_types::DialectEvaluationTestCase {
        description: "invalid query is missing evidence rather than a passing contract",
        dialect: "duckdb",
        query_sql: "SELECT (",
        expected_code: "SQBRCONTRACT101",
    }];
    for test_case in test_cases {
        let config = json!({"select": ["SQBRCONTRACT101"], "cache": {"enabled": false}});
        let mut request: Value = serde_json::from_str(&helpers::request(&project_dir, &config))
            .map_err(|error| error.to_string())?;
        request["models"][0]["query_sql"] = json!(test_case.query_sql);
        request["dialect"] = json!(test_case.dialect);
        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        assert_eq!(
            result["faults"][0]["code"], test_case.expected_code,
            "{}",
            test_case.description
        );
        assert_eq!(
            result["faults"][0]["unevaluated"], true,
            "{}",
            test_case.description
        );
        assert_eq!(
            result["faults"][0]["path"], request["models"][0]["relative_path"],
            "{}",
            test_case.description
        );
        assert!(
            result["faults"][0]["message"]
                .as_str()
                .is_some_and(|message| message.contains("could not be evaluated")),
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_comment_apostrophes_when_generic_fallback_normalizes_rules_sql_then_model_parses()
-> Result<(), String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let test_cases = [
        test_types::DialectEvaluationTestCase {
            description: "line comment apostrophe before a colon path",
            dialect: "postgres",
            query_sql: "-- don't drop the customer path\nSELECT payload:customer_id AS customer_id FROM orders",
            expected_code: "SQBRCONTRACT101",
        },
        test_types::DialectEvaluationTestCase {
            description: "block comment apostrophe before a colon path",
            dialect: "postgres",
            query_sql: "/* customer's path */ SELECT payload:customer_id AS customer_id FROM orders",
            expected_code: "SQBRCONTRACT101",
        },
        test_types::DialectEvaluationTestCase {
            description: "colon inside a line comment stays commented",
            dialect: "postgres",
            query_sql: "SELECT payload:customer_id AS customer_id -- note: it's nested\nFROM orders",
            expected_code: "SQBRCONTRACT101",
        },
    ];

    for test_case in test_cases {
        let config = json!({"select": ["SQBRCONTRACT101"], "cache": {"enabled": false}});
        let mut request: Value = serde_json::from_str(&helpers::request(&project_dir, &config))
            .map_err(|error| error.to_string())?;
        request["dialect"] = json!(test_case.dialect);
        request["models"][0]["query_sql"] = json!(test_case.query_sql);
        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        assert_eq!(
            result["faults"][0]["code"], test_case.expected_code,
            "{}: {result}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_enforced_contract_outputs_when_evaluating_explicit_type_rule_then_returns_expected_faults()
-> Result<(), String> {
    let test_cases = [
        test_types::ExplicitOutputTypeTestCase {
            description: "proven direct passthrough in final CTE",
            query_sql: "WITH final AS (SELECT order_id FROM orders) SELECT order_id FROM final",
            columns: json!([{"name": "order_id", "type": "INTEGER", "type_proven": true}]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "calculated output without outer cast",
            query_sql: "WITH final AS (SELECT amount + 1 AS total FROM orders) SELECT total FROM final",
            columns: json!([{"name": "total", "type": "INTEGER", "type_proven": true}]),
            contract: "enforced",
            expected_fault_count: 1,
            expected_message_fragment: "calculated without an outer explicit cast",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "proven outer cast",
            query_sql: "WITH final AS (SELECT CAST(amount + 1 AS INTEGER) AS total FROM orders) SELECT total FROM final",
            columns: json!([{"name": "total", "type": "INTEGER", "type_proven": true}]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "exact outer cast establishes type when inference remains unproven",
            query_sql: "SELECT CAST(amount AS INTEGER) AS total FROM orders",
            columns: json!([{"name": "total", "type": "INTEGER", "type_proven": false}]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "outer cast targets a different declared type",
            query_sql: "SELECT CAST(amount AS BIGINT) AS total FROM orders",
            columns: json!([{"name": "total", "type": "INTEGER", "type_proven": true}]),
            contract: "enforced",
            expected_fault_count: 1,
            expected_message_fragment: "which is not its declared contract type",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "unproven direct passthrough",
            query_sql: "SELECT order_id FROM orders",
            columns: json!([{"name": "order_id", "type": "INTEGER", "type_proven": false}]),
            contract: "enforced",
            expected_fault_count: 1,
            expected_message_fragment: "passthrough whose type is not proven",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "contract declarations map by output name rather than declaration order",
            query_sql: "SELECT 'ready' AS label, CAST(1 AS INTEGER) AS order_id",
            columns: json!([
                {"name": "order_id", "type": "INTEGER", "type_proven": true},
                {"name": "label", "type": "", "type_proven": false}
            ]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "wildcard output boundary",
            query_sql: "SELECT * FROM orders",
            columns: json!([{"name": "order_id", "type": "INTEGER", "type_proven": true}]),
            contract: "enforced",
            expected_fault_count: 1,
            expected_message_fragment: "wildcard output",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "proven dependency import wildcard",
            query_sql: "WITH final AS (SELECT * FROM __ref(\"orders\")) SELECT order_id FROM final",
            columns: json!([{"name": "order_id", "type": "INTEGER", "type_proven": true}]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "unproven dependency import wildcard",
            query_sql: "WITH final AS (SELECT * FROM __ref(\"orders\")) SELECT order_id FROM final",
            columns: json!([{"name": "order_id", "type": "INTEGER", "type_proven": false}]),
            contract: "enforced",
            expected_fault_count: 1,
            expected_message_fragment: "wildcard output",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "positional set branches require casts",
            query_sql: "SELECT order_id FROM current_orders UNION ALL SELECT order_id FROM archived_orders",
            columns: json!([{"name": "order_id", "type": "INTEGER", "type_proven": true}]),
            contract: "enforced",
            expected_fault_count: 2,
            expected_message_fragment: "set-operation branch that must cast",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "positional set branches with proven casts",
            query_sql: "SELECT CAST(order_id AS INTEGER) AS order_id FROM current_orders UNION ALL SELECT CAST(order_id AS INTEGER) AS order_id FROM archived_orders",
            columns: json!([{"name": "order_id", "type": "INTEGER", "type_proven": true}]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "exact set branch casts establish types when inference remains unproven",
            query_sql: "SELECT CAST(order_id AS INTEGER) AS order_id FROM current_orders UNION ALL SELECT CAST(order_id AS INTEGER) AS order_id FROM archived_orders",
            columns: json!([{"name": "order_id", "type": "INTEGER", "type_proven": false}]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "by-name set branches map reordered outputs",
            query_sql: "SELECT CAST(order_id AS INTEGER) AS order_id, CAST(amount AS DECIMAL(18, 2)) AS amount FROM current_orders UNION ALL BY NAME SELECT CAST(amount AS DECIMAL(18, 2)) AS amount, CAST(order_id AS INTEGER) AS order_id FROM archived_orders",
            columns: json!([
                {"name": "order_id", "type": "INTEGER", "type_proven": true},
                {"name": "amount", "type": "DECIMAL(18, 2)", "type_proven": true}
            ]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "cast inside conditional is not outer cast",
            query_sql: "SELECT CASE WHEN active THEN CAST(amount AS INTEGER) ELSE 0 END AS total FROM orders",
            columns: json!([{"name": "total", "type": "INTEGER", "type_proven": true}]),
            contract: "enforced",
            expected_fault_count: 1,
            expected_message_fragment: "calculated without an outer explicit cast",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "contract-none model is outside rule scope",
            query_sql: "SELECT amount + 1 AS total FROM orders",
            columns: json!([{"name": "total", "type": "INTEGER", "type_proven": true}]),
            contract: "none",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "renamed and reordered terminal columns map to the final CTE",
            query_sql: "WITH final AS (SELECT CAST(amount AS DECIMAL(18, 2)) AS amount, CAST(order_id AS INTEGER) AS order_id FROM orders) SELECT order_id AS id, amount AS total FROM final",
            columns: json!([
                {"name": "id", "type": "INTEGER", "type_proven": true},
                {"name": "total", "type": "DECIMAL(18, 2)", "type_proven": true}
            ]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "final CTE maps contract declarations by terminal output name",
            query_sql: "WITH final AS (SELECT 'ready' AS label, CAST(1 AS INTEGER) AS order_id) SELECT label, order_id FROM final",
            columns: json!([
                {"name": "order_id", "type": "INTEGER", "type_proven": true},
                {"name": "label", "type": "", "type_proven": false}
            ]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "wildcard terminal maps contract declarations to final CTE output order",
            query_sql: "WITH final AS (SELECT 'ready' AS label, CAST(1 AS INTEGER) AS order_id) SELECT * FROM final",
            columns: json!([
                {"name": "order_id", "type": "INTEGER", "type_proven": true},
                {"name": "label", "type": "", "type_proven": false}
            ]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "wildcard terminal maps explicit final CTE column aliases by name",
            query_sql: "WITH final(order_id, amount) AS (SELECT CAST(raw_id AS INTEGER) AS raw_id, CAST(raw_amount AS DECIMAL(18, 2)) AS raw_amount FROM orders) SELECT * FROM final",
            columns: json!([
                {"name": "amount", "type": "DECIMAL(18, 2)", "type_proven": true},
                {"name": "order_id", "type": "INTEGER", "type_proven": true}
            ]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "renamed and reordered terminal columns map through positional set branches",
            query_sql: "WITH final AS (SELECT CAST(amount AS DECIMAL(18, 2)) AS amount, CAST(order_id AS INTEGER) AS order_id FROM current_orders UNION ALL SELECT CAST(amount AS DECIMAL(18, 2)) AS amount, CAST(order_id AS INTEGER) AS order_id FROM archived_orders) SELECT order_id AS id, amount AS total FROM final",
            columns: json!([
                {"name": "id", "type": "INTEGER", "type_proven": true},
                {"name": "total", "type": "DECIMAL(18, 2)", "type_proven": true}
            ]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "terminal subset ignores extra final CTE outputs",
            query_sql: "WITH final AS (SELECT CAST(order_id AS INTEGER) AS order_id, ignored_count + 1, ignored_label, CAST(amount AS DECIMAL(18, 2)) AS amount FROM orders) SELECT order_id AS id, amount AS total FROM final",
            columns: json!([
                {"name": "id", "type": "INTEGER", "type_proven": true},
                {"name": "total", "type": "DECIMAL(18, 2)", "type_proven": true}
            ]),
            contract: "enforced",
            expected_fault_count: 0,
            expected_message_fragment: "",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "terminal subset reports the selected calculated final CTE output",
            query_sql: "WITH final AS (SELECT CAST(order_id AS INTEGER) AS order_id, ignored_count, ignored_label, amount + 1 AS amount FROM orders) SELECT order_id AS id, amount AS total FROM final",
            columns: json!([
                {"name": "id", "type": "INTEGER", "type_proven": true},
                {"name": "total", "type": "DECIMAL(18, 2)", "type_proven": true}
            ]),
            contract: "enforced",
            expected_fault_count: 1,
            expected_message_fragment: "output \"amount\" is calculated without an outer explicit cast",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "terminal subset maps outputs introduced by right by-name branch",
            query_sql: "WITH final AS (SELECT CAST(1 AS INTEGER) AS ignored UNION ALL BY NAME SELECT 2 + 1 AS amount) SELECT amount AS total FROM final",
            columns: json!([
                {"name": "total", "type": "INTEGER", "type_proven": true}
            ]),
            contract: "enforced",
            expected_fault_count: 1,
            expected_message_fragment: "output \"amount\" is calculated without an outer explicit cast",
        },
        test_types::ExplicitOutputTypeTestCase {
            description: "terminal subset maps explicit final CTE column aliases",
            query_sql: "WITH final(order_id, ignored) AS (SELECT 1 + 1 AS raw_id, 3 AS extra) SELECT order_id AS id FROM final",
            columns: json!([
                {"name": "id", "type": "INTEGER", "type_proven": true}
            ]),
            contract: "enforced",
            expected_fault_count: 1,
            expected_message_fragment: "output \"raw_id\" is calculated without an outer explicit cast",
        },
    ];
    for test_case in test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let config = json!({"select": ["SQBRCONTRACT105"], "cache": {"enabled": false}});
        let mut request: Value = serde_json::from_str(&helpers::request(&project_dir, &config))
            .map_err(|error| error.to_string())?;
        request["models"][0]["query_sql"] = json!(test_case.query_sql);
        request["models"][0]["authored_sql"] = json!(test_case.query_sql);
        request["models"][0]["config"] = json!({"contract": test_case.contract});
        request["models"][0]["columns"] = test_case.columns;

        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        let faults = result["faults"]
            .as_array()
            .ok_or("faults must be an array")?;
        assert_eq!(
            faults.len(),
            test_case.expected_fault_count,
            "{}",
            test_case.description
        );
        assert!(
            faults.iter().all(|fault| fault["message"]
                .as_str()
                .is_some_and(|message| message.contains(test_case.expected_message_fragment))),
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_compiler_proven_duckdb_dynamic_pivot_when_evaluating_contract_rules_then_skips_query_parse()
-> Result<(), String> {
    let test_cases = [test_types::DynamicPivotEvaluationTestCase {
        description: "compiler-proven bare DuckDB pivot",
        expected_faults: json!([]),
    }];
    for test_case in test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let config = json!({
            "select": ["SQBRMODEL102", "SQBRCONTRACT101", "SQBRCONTRACT105", "SQBRCONTRACT106"],
            "cache": {"enabled": false}
        });
        let mut request: Value = serde_json::from_str(&helpers::request(&project_dir, &config))
            .map_err(|error| error.to_string())?;
        request["dialect"] = json!("duckdb");
        request["models"][0]["query_sql"] =
            json!("PIVOT orders ON category USING MAX(amount) GROUP BY customer_id");
        request["models"][0]["authored_sql"] = request["models"][0]["query_sql"].clone();
        request["models"][0]["config"] = json!({"contract": "enforced"});
        request["models"][0]["columns"] = json!([
            {"name": "customer_id", "type": "INTEGER", "type_proven": true}
        ]);
        request["models"][0]["dynamic_columns"] = json!([{
            "name": "category_amounts",
            "pivot_column": "category",
            "value_column": "amount",
            "aggregate": "MAX",
            "type": "DECIMAL(12,2)",
            "type_proven": true
        }]);
        request["models"][0]["dynamic_columns_proven"] = json!(true);
        request["models"][0]["bare_dynamic_pivot"] = json!(true);

        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            result["faults"], test_case.expected_faults,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_contract_name_type_options_when_evaluating_then_accepts_configured_interfaces()
-> Result<(), String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let test_cases = [
        test_types::ContractNameTypeOptionsTestCase {
            description: "configured compatibility types are accepted",
            config: json!({
                "select": ["SQBRCONTRACT102", "SQBRCONTRACT103", "SQBRCONTRACT104"],
                "rule_options": {
                    "SQBRCONTRACT102": {"allow_numeric_indicators": true},
                    "SQBRCONTRACT103": {
                        "allow_date_for_at": true,
                        "allow_numeric_epoch": true,
                        "allow_encoded_values": true
                    },
                    "SQBRCONTRACT104": {
                        "allow_timestamps": true,
                        "allow_encoded_values": true
                    }
                },
                "cache": {"enabled": false}
            }),
            expected_fault_codes: &[],
        },
        test_types::ContractNameTypeOptionsTestCase {
            description: "compatibility types remain findings by default",
            config: json!({
                "select": ["SQBRCONTRACT102", "SQBRCONTRACT103", "SQBRCONTRACT104"],
                "cache": {"enabled": false}
            }),
            expected_fault_codes: &[
                "SQBRCONTRACT102",
                "SQBRCONTRACT103",
                "SQBRCONTRACT103",
                "SQBRCONTRACT103",
                "SQBRCONTRACT104",
                "SQBRCONTRACT104",
            ],
        },
    ];

    for test_case in test_cases {
        let mut request: Value =
            serde_json::from_str(&helpers::request(&project_dir, &test_case.config))
                .map_err(|error| error.to_string())?;
        request["models"][0]["query_sql"] = json!(
            "SELECT is_ready, first_seen_at, source_timestamp, opaque_ts, raw_date, observed_date FROM orders"
        );
        request["models"][0]["columns"] = json!([
            {"name": "is_ready", "type": "INTEGER"},
            {"name": "first_seen_at", "type": "DATE"},
            {"name": "source_timestamp", "type": "NUMBER(38,0)"},
            {"name": "opaque_ts", "type": "VARIANT"},
            {"name": "raw_date", "type": "VARCHAR"},
            {"name": "observed_date", "type": "TIMESTAMP_NTZ"}
        ]);

        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        let fault_codes: Vec<&str> = result["faults"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|fault| fault["code"].as_str())
            .collect();
        assert_eq!(
            fault_codes, test_case.expected_fault_codes,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_numeric_comparisons_when_evaluating_named_decisions_then_only_authored_decisions_fault()
-> Result<(), String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let test_cases = [
        test_types::NumericDecisionTestCase {
            description: "authored threshold is a named decision finding",
            query_sql: "SELECT amount FROM orders WHERE amount > 5",
            authored_sql: "SELECT amount FROM orders WHERE amount > 5",
            expected_fault_count: 1,
        },
        test_types::NumericDecisionTestCase {
            description: "expanded named constant is not an authored literal finding",
            query_sql: "SELECT amount FROM orders WHERE amount > 5",
            authored_sql: "SELECT amount FROM orders WHERE amount > @const(\"_minimum_amount\")",
            expected_fault_count: 0,
        },
        test_types::NumericDecisionTestCase {
            description: "numeric comparison shown only in a comment is not an authored finding",
            query_sql: "SELECT amount FROM orders WHERE amount > 5",
            authored_sql: concat!(
                "-- The expanded predicate is amount > 5.\n",
                "SELECT amount FROM orders WHERE amount > @const(\"_minimum_amount\")"
            ),
            expected_fault_count: 0,
        },
        test_types::NumericDecisionTestCase {
            description: "fixed output bucket suffix makes equality structural",
            query_sql: concat!(
                "SELECT MAX(CASE WHEN offset_seconds = 30 THEN amount END) AS amount_30 ",
                "FROM orders"
            ),
            authored_sql: concat!(
                "SELECT MAX(CASE WHEN offset_seconds = 30 THEN amount END) AS amount_30 ",
                "FROM orders"
            ),
            expected_fault_count: 0,
        },
        test_types::NumericDecisionTestCase {
            description: "output suffix does not exempt a threshold comparison",
            query_sql: ("SELECT CASE WHEN amount > 30 THEN 'large' END AS amount_30 FROM orders"),
            authored_sql: ("SELECT CASE WHEN amount > 30 THEN 'large' END AS amount_30 FROM orders"),
            expected_fault_count: 1,
        },
        test_types::NumericDecisionTestCase {
            description: "authored between bounds are a named decision finding",
            query_sql: "SELECT code FROM orders WHERE TRY_CAST(code AS INTEGER) BETWEEN 30 AND 61",
            authored_sql: "SELECT code FROM orders WHERE TRY_CAST(code AS INTEGER) BETWEEN 30 AND 61",
            expected_fault_count: 1,
        },
        test_types::NumericDecisionTestCase {
            description: "authored not between bounds are a named decision finding",
            query_sql: "SELECT code FROM orders WHERE amount NOT BETWEEN 5 AND 10",
            authored_sql: "SELECT code FROM orders WHERE amount NOT BETWEEN 5 AND 10",
            expected_fault_count: 1,
        },
        test_types::NumericDecisionTestCase {
            description: "canonical between bounds pass",
            query_sql: "SELECT code FROM orders WHERE amount BETWEEN 0 AND 1",
            authored_sql: "SELECT code FROM orders WHERE amount BETWEEN 0 AND 1",
            expected_fault_count: 0,
        },
        test_types::NumericDecisionTestCase {
            description: "expanded between constants are not authored literal findings",
            query_sql: "SELECT code FROM orders WHERE amount BETWEEN 30 AND 61",
            authored_sql: "SELECT code FROM orders WHERE amount BETWEEN @const(\"_low\") AND @const(\"_high\")",
            expected_fault_count: 0,
        },
        test_types::NumericDecisionTestCase {
            description: "authored numeric in list is a named decision finding",
            query_sql: "SELECT code FROM orders WHERE TRY_CAST(code AS INTEGER) IN (30, 31, 45)",
            authored_sql: "SELECT code FROM orders WHERE TRY_CAST(code AS INTEGER) IN (30, 31, 45)",
            expected_fault_count: 1,
        },
        test_types::NumericDecisionTestCase {
            description: "authored numeric not in list is a named decision finding",
            query_sql: "SELECT code FROM orders WHERE amount NOT IN (30, 31)",
            authored_sql: "SELECT code FROM orders WHERE amount NOT IN (30, 31)",
            expected_fault_count: 1,
        },
        test_types::NumericDecisionTestCase {
            description: "canonical numeric in list passes",
            query_sql: "SELECT code FROM orders WHERE amount IN (-1, 0, 1)",
            authored_sql: "SELECT code FROM orders WHERE amount IN (-1, 0, 1)",
            expected_fault_count: 0,
        },
        test_types::NumericDecisionTestCase {
            description: "expanded list constant is not an authored literal finding",
            query_sql: "SELECT code FROM orders WHERE amount IN (30, 31, 45)",
            authored_sql: "SELECT code FROM orders WHERE amount IN @const(\"_codes\")",
            expected_fault_count: 0,
        },
        test_types::NumericDecisionTestCase {
            description: "between in projected case is a named decision finding",
            query_sql: "SELECT CASE WHEN amount BETWEEN 30 AND 61 THEN 'mid' END AS band FROM orders",
            authored_sql: "SELECT CASE WHEN amount BETWEEN 30 AND 61 THEN 'mid' END AS band FROM orders",
            expected_fault_count: 1,
        },
        test_types::NumericDecisionTestCase {
            description: "nested comparison reports only its leaf decision",
            query_sql: ("SELECT CASE WHEN amount > 5 THEN status END = 'ready' AS selected FROM orders"),
            authored_sql: ("SELECT CASE WHEN amount > 5 THEN status END = 'ready' AS selected FROM orders"),
            expected_fault_count: 1,
        },
    ];

    for test_case in test_cases {
        let mut request: Value = serde_json::from_str(&helpers::request(
            &project_dir,
            &json!({
                "select": ["SQBRDECLARATION102"],
                "cache": {"enabled": false}
            }),
        ))
        .map_err(|error| error.to_string())?;
        request["models"][0]["query_sql"] = json!(test_case.query_sql);
        request["models"][0]["authored_sql"] = json!(test_case.authored_sql);

        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        let faults = result["faults"]
            .as_array()
            .ok_or_else(|| "faults must be an array".to_string())?;
        assert_eq!(
            faults.len(),
            test_case.expected_fault_count,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_model_sql_when_stubbing_shape_then_offsets_map_back_to_source_positions()
-> Result<(), String> {
    let test_cases = [test_types::ShapeTestCase {
        description: "calls in code are stubbed by occurrence; quoted and commented calls stay",
        dialect: "snowflake",
        sql: concat!(
            "SELECT '__ref(\"quoted\")' AS q, /* __ref(\"comment\") */ o.payload:key::STRING AS k\n",
            "FROM __ref( \"orders\" ) AS o JOIN __source(\"raw\", \"items\") AS i ON i.id = o.id"
        ),
        expected_calls: &["__ref(\"orders\")", "__source(\"raw\",\"items\")"],
        expected_fragment: "i.id = o.id",
    }];

    for test_case in test_cases {
        let shape = model_shape(test_case.sql, test_case.dialect)?;
        let calls: Vec<&str> = shape.calls.iter().map(|call| call.text.as_str()).collect();
        assert_eq!(calls, test_case.expected_calls, "{}", test_case.description);
        assert!(
            shape.sql.contains("'__ref(\"quoted\")'"),
            "{}",
            test_case.description
        );
        let normalized_start = shape
            .sql
            .find(test_case.expected_fragment)
            .ok_or("fragment missing from the normalized SQL")?;
        let normalized_start = shape.sql[..normalized_start].chars().count();
        let original_start = test_case
            .sql
            .find(test_case.expected_fragment)
            .ok_or("fragment missing from the model SQL")?;
        let source = ModelSource::new(test_case.sql, &shape.offsets, (0, 0));
        let end = normalized_start + test_case.expected_fragment.chars().count();
        assert_eq!(
            source.text(normalized_start, end),
            test_case.expected_fragment,
            "{}",
            test_case.description
        );
        let line_start = test_case.sql[..original_start]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        assert_eq!(
            source.line_column(normalized_start),
            (2, (original_start - line_start + 1) as u64),
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_deep_valid_expression_when_evaluating_rules_then_parser_budget_is_sufficient()
-> Result<(), String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let test_cases = [test_types::DeepExpressionTestCase {
        description: "deep valid expression remains within the parser budget",
        depth: 55,
        expected_code: "SQBRCONTRACT101",
    }];

    for test_case in test_cases {
        let config = json!({"select": ["SQBRCONTRACT101"], "cache": {"enabled": false}});
        let mut request: Value = serde_json::from_str(&helpers::request(&project_dir, &config))
            .map_err(|error| error.to_string())?;
        let expression = format!(
            "{}1{}",
            "COALESCE(".repeat(test_case.depth),
            ", 0)".repeat(test_case.depth)
        );
        request["models"][0]["query_sql"] = json!(format!("SELECT {expression} AS value"));
        let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
            .map_err(|error| error.to_string())?;
        assert_eq!(
            result["faults"][0]["code"], test_case.expected_code,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_path_threshold_overrides_when_evaluating_then_matches_in_authored_order()
-> Result<(), String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let test_cases = [
        test_types::ThresholdEvaluationTestCase {
            description: "matching override raises both minima",
            config: json!({
                "select": ["SQBRTEST201", "SQBRTEST202"],
                "thresholds": {"min_audits_per_model": 1, "min_tests_per_model": 1},
                "threshold_overrides": [{
                    "paths": ["models/mart/**"],
                    "thresholds": {"min_audits_per_model": 2, "min_tests_per_model": 2},
                    "reason": "marts require stronger evidence"
                }],
                "cache": {"enabled": false}
            }),
            query_sql: "SELECT id + 1 AS id FROM orders",
            references: json!([]),
            expected_codes: &["SQBRTEST201", "SQBRTEST202"],
        },
        test_types::ThresholdEvaluationTestCase {
            description: "near miss keeps global minima",
            config: json!({
                "select": ["SQBRTEST201", "SQBRTEST202"],
                "thresholds": {"min_audits_per_model": 1, "min_tests_per_model": 1},
                "threshold_overrides": [{
                    "paths": ["models/staging/**"],
                    "thresholds": {"min_audits_per_model": 2, "min_tests_per_model": 2},
                    "reason": "staging requires stronger evidence"
                }],
                "cache": {"enabled": false}
            }),
            query_sql: "SELECT id + 1 AS id FROM orders",
            references: json!([]),
            expected_codes: &[],
        },
        test_types::ThresholdEvaluationTestCase {
            description: "last matching override wins",
            config: json!({
                "select": ["SQBRTEST201", "SQBRTEST202"],
                "thresholds": {"min_audits_per_model": 1, "min_tests_per_model": 1},
                "threshold_overrides": [
                    {
                        "paths": ["models/**"],
                        "thresholds": {"min_audits_per_model": 2, "min_tests_per_model": 2},
                        "reason": "all models require stronger evidence"
                    },
                    {
                        "paths": ["models/mart/**"],
                        "thresholds": {"min_audits_per_model": 1, "min_tests_per_model": 1},
                        "reason": "mart migration temporarily uses global minima"
                    }
                ],
                "cache": {"enabled": false}
            }),
            query_sql: "SELECT id + 1 AS id FROM orders",
            references: json!([]),
            expected_codes: &[],
        },
        test_types::ThresholdEvaluationTestCase {
            description: "audit and test overrides resolve independently",
            config: json!({
                "select": ["SQBRTEST201", "SQBRTEST202"],
                "thresholds": {"min_audits_per_model": 1, "min_tests_per_model": 1},
                "threshold_overrides": [{
                    "paths": ["models/mart/**"],
                    "thresholds": {"min_audits_per_model": 2},
                    "reason": "marts require an additional audit"
                }],
                "cache": {"enabled": false}
            }),
            query_sql: "SELECT id + 1 AS id FROM orders",
            references: json!([]),
            expected_codes: &["SQBRTEST201"],
        },
        test_types::ThresholdEvaluationTestCase {
            description: "matching override preserves passthrough exemption",
            config: json!({
                "select": ["SQBRTEST201", "SQBRTEST202"],
                "threshold_overrides": [{
                    "paths": ["models/mart/**"],
                    "thresholds": {"min_audits_per_model": 10, "min_tests_per_model": 10},
                    "reason": "marts require strong evidence"
                }],
                "cache": {"enabled": false}
            }),
            query_sql: "WITH upstream AS (SELECT * FROM __ref(\"commerce__stg__orders\")) SELECT id FROM upstream",
            references: json!([{"ref_kind": "ref", "ref_name": "commerce__stg__orders"}]),
            expected_codes: &[],
        },
    ];

    for test_case in &test_cases {
        let request_json = helpers::threshold_request(
            &project_dir,
            &test_case.config,
            test_case.query_sql,
            &test_case.references,
        );
        let result: Value = serde_json::from_str(&evaluate_json(&request_json)?)
            .map_err(|error| error.to_string())?;
        let codes: Vec<&str> = result["faults"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|fault| fault["code"].as_str())
            .collect();
        assert_eq!(codes, test_case.expected_codes, "{}", test_case.description);
    }
    Ok(())
}

#[test]
fn given_threshold_override_change_when_evaluating_then_ruleset_fingerprint_changes()
-> Result<(), String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let test_cases = [test_types::ThresholdFingerprintTestCase {
        description: "path threshold changes the ruleset fingerprint",
        base_config: json!({
            "select": ["SQBRTEST202"],
            "cache": {"enabled": false}
        }),
        overridden_config: json!({
            "select": ["SQBRTEST202"],
            "threshold_overrides": [{
                "paths": ["models/mart/**"],
                "thresholds": {"min_tests_per_model": 2},
                "reason": "marts require two focused tests"
            }],
            "cache": {"enabled": false}
        }),
        expected_different: true,
    }];

    for test_case in &test_cases {
        let references = json!([]);
        let base_request = helpers::threshold_request(
            &project_dir,
            &test_case.base_config,
            "SELECT id + 1 AS id FROM orders",
            &references,
        );
        let overridden_request = helpers::threshold_request(
            &project_dir,
            &test_case.overridden_config,
            "SELECT id + 1 AS id FROM orders",
            &references,
        );
        let base_result: Value = serde_json::from_str(&evaluate_json(&base_request)?)
            .map_err(|error| error.to_string())?;
        let overridden_result: Value = serde_json::from_str(&evaluate_json(&overridden_request)?)
            .map_err(|error| error.to_string())?;
        let different =
            base_result["ruleset_fingerprint"] != overridden_result["ruleset_fingerprint"];

        assert_eq!(
            different, test_case.expected_different,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_native_build_when_reading_cache_identity_then_it_is_a_source_hash() {
    let test_cases = [test_types::BuildIdentityTestCase {
        description: "cache identity is a source content hash",
        expected_hash_length: 16,
    }];
    for test_case in &test_cases {
        let hash = NATIVE_BUILD_IDENTITY;

        assert!(
            hash.len() == test_case.expected_hash_length
                && hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_workspace_checkout_when_deriving_build_identity_then_workspace_files_are_hashed() {
    let test_cases = [
        test_types::HashedWorkspaceFileTestCase {
            description: "dependency features and build profiles change the identity",
            workspace_file: "../../Cargo.toml",
            expected_hashed: true,
        },
        test_types::HashedWorkspaceFileTestCase {
            description: "resolved dependency versions change the identity",
            workspace_file: "../../Cargo.lock",
            expected_hashed: true,
        },
    ];
    for test_case in &test_cases {
        let hashed = env!("SQLBUILD_NATIVE_HASHED_WORKSPACE_FILES")
            .split(',')
            .any(|file| file == test_case.workspace_file);
        assert_eq!(
            hashed, test_case.expected_hashed,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_model_whose_analysis_was_skipped_when_building_rules_rows_then_its_rules_are_off()
-> Result<(), String> {
    let test_cases = [
        test_types::AnalysisSkipTestCase {
            description: "an analysed model keeps its rules",
            sql_analysis_skipped: false,
            config: &[],
            expected_disabled: false,
        },
        test_types::AnalysisSkipTestCase {
            description: "a model its dialect's native parser cannot read gets no rules",
            sql_analysis_skipped: true,
            config: &[],
            expected_disabled: true,
        },
        test_types::AnalysisSkipTestCase {
            description: "a model that opts out with sql_analysis false gets no rules",
            sql_analysis_skipped: false,
            config: &[("sql_analysis", false)],
            expected_disabled: true,
        },
    ];
    for test_case in test_cases {
        let mut row: ModelRow = ModelRow {
            name: "orders".to_owned(),
            relative_path: "models/orders.sql".to_owned(),
            query_sql: "SELECT 1 AS id".to_owned(),
            sql_analysis_skipped: test_case.sql_analysis_skipped,
            ..ModelRow::default()
        };
        row.config.extend(
            test_case
                .config
                .iter()
                .map(|(key, enabled)| ((*key).to_owned(), Value::Bool(*enabled))),
        );
        let context: ModelRowsContext<'_> = ModelRowsContext {
            sql_analysis_enabled: true,
            include_type_proof: false,
            dialect: "duckdb",
            attached_audit_targets: &[],
            sql_tests: &[],
        };
        let models: Vec<Model> = build_models(vec![row], context, |_, _| Ok(false))?;
        assert_eq!(
            models
                .iter()
                .map(|model| model.sql_analysis_disabled)
                .collect::<Vec<bool>>(),
            vec![test_case.expected_disabled],
            "{}",
            test_case.description
        );
    }
    Ok(())
}
