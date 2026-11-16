use std::collections::BTreeMap;

use serde_json::json;

use crate::engine::tests::helpers::{
    built_json_grains, grain_column, grain_keys, json_grain_faults, payload_grain,
};
use crate::engine::tests::test_types::{JsonGrainTestCase, JsonGrainsBuildTestCase};
use crate::models::JsonGrain;

const RULE: &[&str] = &["SQBRPERFORMANCE102"];
const LINES_SQL: &str = "SELECT l.order_id, l.line_no, o.payload\nFROM __ref(\"order_lines\") l JOIN __ref(\"orders\") o USING (order_id)";

#[test]
fn given_grain_facts_when_evaluating_json_finer_grain_then_only_coarser_inputs_are_flagged()
-> Result<(), String> {
    let test_cases = [
        JsonGrainTestCase {
            description: "a payload keyed by order copied onto order lines is flagged at its column",
            dialect: "snowflake",
            name: "order_lines_enriched",
            select: RULE,
            query_sql: LINES_SQL,
            json_grain: payload_grain(json!(["order_id"]), json!([])),
            expected_faults: &[(
                "SQBRPERFORMANCE102",
                1,
                31,
                "payload copies VARIANT from orders, one row per (order_id), onto rows keyed by (order_id, line_no)",
            )],
        },
        JsonGrainTestCase {
            description: "key columns match whatever their case and order",
            dialect: "snowflake",
            name: "order_lines_enriched",
            select: RULE,
            query_sql: LINES_SQL,
            json_grain: json!({
                "model_keys": [["LINE_NO", "Order_Id"]],
                "columns": [{"name": "PAYLOAD", "data_type": "VARIANT", "inputs": ["ref:orders"]}],
                "input_keys": {"ref:orders": [["ORDER_ID"]]},
            }),
            expected_faults: &[("SQBRPERFORMANCE102", 1, 31, "PAYLOAD copies VARIANT")],
        },
        JsonGrainTestCase {
            description: "an input on the model's own grain repeats nothing",
            dialect: "snowflake",
            name: "order_lines_enriched",
            select: RULE,
            query_sql: LINES_SQL,
            json_grain: payload_grain(json!(["line_no", "order_id"]), json!([])),
            expected_faults: &[],
        },
        JsonGrainTestCase {
            description: "another declared model key on the input's grain proves one row per input row",
            dialect: "snowflake",
            name: "order_payload_dated",
            select: RULE,
            query_sql: LINES_SQL,
            json_grain: json!({
                "model_keys": [["order_id", "line_no"], ["ORDER_ID"]],
                "columns": [{"name": "payload", "data_type": "VARIANT", "inputs": ["ref:orders"]}],
                "input_keys": {"ref:orders": [["order_id"]]},
            }),
            expected_faults: &[],
        },
        JsonGrainTestCase {
            description: "an input with two keys is not coarser when either covers a model key",
            dialect: "snowflake",
            name: "order_payload_dated",
            select: RULE,
            query_sql: LINES_SQL,
            json_grain: json!({
                "model_keys": [["order_number", "order_date"], ["order_id"]],
                "columns": [{"name": "payload", "data_type": "VARIANT", "inputs": ["ref:orders"]}],
                "input_keys": {"ref:orders": [["order_id"], ["order_number"]]},
            }),
            expected_faults: &[],
        },
        JsonGrainTestCase {
            description: "an input keyed by another column is not coarser",
            dialect: "snowflake",
            name: "order_lines_enriched",
            select: RULE,
            query_sql: LINES_SQL,
            json_grain: payload_grain(json!(["customer_id"]), json!([])),
            expected_faults: &[],
        },
        JsonGrainTestCase {
            description: "a model whose only reader is the published view of its own entity is exempt",
            dialect: "snowflake",
            name: "shop__mart__orders",
            select: RULE,
            query_sql: LINES_SQL,
            json_grain: payload_grain(json!(["order_id"]), json!(["shop__mart_v__orders"])),
            expected_faults: &[],
        },
        JsonGrainTestCase {
            description: "a published model that another model also reads is not exempt",
            dialect: "snowflake",
            name: "shop__mart__orders",
            select: RULE,
            query_sql: LINES_SQL,
            json_grain: payload_grain(
                json!(["order_id"]),
                json!(["shop__int_enriched__orders", "shop__mart_v__orders"]),
            ),
            expected_faults: &[("SQBRPERFORMANCE102", 1, 31, "payload copies VARIANT")],
        },
        JsonGrainTestCase {
            description: "a view of another entity does not exempt the model",
            dialect: "snowflake",
            name: "shop__mart__orders",
            select: RULE,
            query_sql: LINES_SQL,
            json_grain: payload_grain(json!(["order_id"]), json!(["shop__mart_v__customers"])),
            expected_faults: &[("SQBRPERFORMANCE102", 1, 31, "payload copies VARIANT")],
        },
        JsonGrainTestCase {
            description: "a model without lineage is skipped, not flagged",
            dialect: "snowflake",
            name: "order_lines_enriched",
            select: RULE,
            query_sql: LINES_SQL,
            json_grain: json!({"model_keys": [["order_id", "line_no"]], "skipped": "no column lineage"}),
            expected_faults: &[],
        },
        JsonGrainTestCase {
            description: "the rule runs only on Snowflake",
            dialect: "duckdb",
            name: "order_lines_enriched",
            select: RULE,
            query_sql: LINES_SQL,
            json_grain: payload_grain(json!(["order_id"]), json!([])),
            expected_faults: &[],
        },
    ];
    for test_case in test_cases {
        let faults = json_grain_faults(&test_case)?;
        let observed: Vec<(&str, u64, u64, &str)> = faults
            .iter()
            .enumerate()
            .map(|(index, (code, line, column, message))| {
                let fragment = test_case
                    .expected_faults
                    .get(index)
                    .map(|expected| expected.3)
                    .filter(|fragment| message.contains(fragment))
                    .unwrap_or(message.as_str());
                (code.as_str(), *line, *column, fragment)
            })
            .collect();
        assert_eq!(
            observed, test_case.expected_faults,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_rows_lineage_and_keys_when_building_grains_then_only_copied_semi_structured_columns_are_kept()
 {
    let test_cases = [
        JsonGrainsBuildTestCase {
            description: "a keyed model keeps only copied VARIANT, OBJECT and ARRAY columns with their input keys",
            row_name: "order_lines_enriched",
            expected_grain: Some(JsonGrain {
                model_keys: grain_keys(&["order_id", "line_no"]),
                columns: vec![
                    grain_column("payload", "VARIANT", &["ref:orders"]),
                    grain_column("items", "ARRAY(VARCHAR)", &["source:shop.raw_orders"]),
                    grain_column("region", "object", &["seed:countries"]),
                ],
                input_keys: BTreeMap::from([
                    ("ref:orders".to_owned(), grain_keys(&["order_id"])),
                    ("seed:countries".to_owned(), grain_keys(&["country_code"])),
                ]),
                readers: Vec::new(),
                skipped: None,
            }),
        },
        JsonGrainsBuildTestCase {
            description: "a model without lineage is skipped and lists its readers",
            row_name: "orders",
            expected_grain: Some(JsonGrain {
                model_keys: grain_keys(&["order_id"]),
                readers: vec!["order_lines_enriched".to_owned(), "unkeyed".to_owned()],
                skipped: Some("no column lineage".to_owned()),
                ..JsonGrain::default()
            }),
        },
        JsonGrainsBuildTestCase {
            description: "a model without a declared key has no grain",
            row_name: "unkeyed",
            expected_grain: None,
        },
        JsonGrainsBuildTestCase {
            description: "a model without inferred types is skipped",
            row_name: "untyped",
            expected_grain: Some(JsonGrain {
                model_keys: grain_keys(&["id"]),
                skipped: Some("no inferred column types".to_owned()),
                ..JsonGrain::default()
            }),
        },
    ];
    let grains: BTreeMap<String, Option<JsonGrain>> = built_json_grains();
    for test_case in test_cases {
        assert_eq!(
            grains.get(test_case.row_name),
            Some(&test_case.expected_grain),
            "{}",
            test_case.description
        );
    }
}
