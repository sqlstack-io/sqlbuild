use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::engine::main::json_grains::json_grains;
use crate::engine::tests::helpers::json_grain_faults;
use crate::engine::tests::test_types::JsonGrainTestCase;
use crate::models::{GrainColumn, JsonGrain, LineageColumn, ModelRow, Reference};

const RULE: &[&str] = &["SQBRPERFORMANCE102"];
const LINES_SQL: &str = "SELECT l.order_id, l.line_no, o.payload\nFROM __ref(\"order_lines\") l JOIN __ref(\"orders\") o USING (order_id)";

fn payload_grain(input_key: Value, readers: Value) -> Value {
    json!({
        "model_keys": [["order_id", "line_no"]],
        "columns": [{"name": "payload", "data_type": "VARIANT", "inputs": ["ref:orders"]}],
        "input_keys": {"ref:orders": [input_key]},
        "readers": readers,
    })
}

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
        let observed: Vec<(&str, u64, u64, bool)> = faults
            .iter()
            .map(|(code, line, column, _)| (code.as_str(), *line, *column, true))
            .collect();
        let expected: Vec<(&str, u64, u64, bool)> = test_case
            .expected_faults
            .iter()
            .map(|(code, line, column, _)| (*code, *line, *column, true))
            .collect();
        assert_eq!(observed, expected, "{}", test_case.description);
        for ((_, _, _, message), (_, _, _, fragment)) in
            faults.iter().zip(test_case.expected_faults)
        {
            assert!(
                message.contains(fragment),
                "{}: {message}",
                test_case.description
            );
        }
    }
    Ok(())
}

fn row(name: &str, references: &[&str], inferred: &[(&str, &str)]) -> ModelRow {
    ModelRow {
        name: name.to_owned(),
        references: references
            .iter()
            .map(|name| Reference {
                ref_kind: "ref".to_owned(),
                ref_name: (*name).to_owned(),
                ref_package: None,
            })
            .collect(),
        inferred_columns: inferred
            .iter()
            .map(|(name, kind)| ((*name).to_owned(), Some((*kind).to_owned())))
            .collect(),
        ..ModelRow::default()
    }
}

fn lineage(output: &str, sources: &[(&str, &str)], aggregated: bool) -> LineageColumn {
    LineageColumn {
        output: output.to_owned(),
        sources: sources
            .iter()
            .map(|(kind, name)| ((*kind).to_owned(), (*name).to_owned()))
            .collect(),
        aggregated,
    }
}

fn keys(key: &[&str]) -> Vec<Vec<String>> {
    vec![key.iter().map(|column| (*column).to_owned()).collect()]
}

#[test]
fn given_rows_lineage_and_keys_when_building_grains_then_only_copied_semi_structured_columns_are_kept()
 {
    let mut rows = vec![
        row(
            "order_lines_enriched",
            &["orders", "order_lines"],
            &[
                ("payload", "VARIANT"),
                ("items", "ARRAY(VARCHAR)"),
                ("region", "object"),
                ("total", "NUMBER(38,0)"),
                ("payloads", "ARRAY"),
                ("arrayish", "ARRAYLIKE"),
            ],
        ),
        row("orders", &[], &[("payload", "VARIANT")]),
        row("unkeyed", &["orders"], &[("payload", "VARIANT")]),
        row("untyped", &[], &[]),
    ];
    let lineage = vec![
        Some(vec![
            lineage(
                "PAYLOAD",
                &[("model", "orders"), ("table_fn", "explode")],
                false,
            ),
            lineage("items", &[("source", "shop.raw_orders")], false),
            lineage("region", &[("seed", "countries")], false),
            lineage("total", &[("model", "orders")], false),
            lineage("payloads", &[("model", "orders")], true),
            lineage("arrayish", &[("model", "orders")], false),
        ]),
        None,
        Some(vec![lineage("payload", &[("model", "orders")], false)]),
        Some(vec![lineage("payload", &[("model", "orders")], false)]),
    ];
    let declared: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::from([
        (
            "ref:order_lines_enriched".to_owned(),
            keys(&["order_id", "line_no"]),
        ),
        ("ref:orders".to_owned(), keys(&["order_id"])),
        ("ref:untyped".to_owned(), keys(&["id"])),
        ("seed:countries".to_owned(), keys(&["country_code"])),
    ]);
    json_grains(&mut rows, lineage, &declared);
    let column = |name: &str, data_type: &str, inputs: &[&str]| GrainColumn {
        name: name.to_owned(),
        data_type: Some(data_type.to_owned()),
        inputs: inputs.iter().map(|input| (*input).to_owned()).collect(),
    };
    let grains: Vec<Option<JsonGrain>> = rows.into_iter().map(|row| row.json_grain).collect();
    assert_eq!(
        grains,
        vec![
            Some(JsonGrain {
                model_keys: keys(&["order_id", "line_no"]),
                columns: vec![
                    column("payload", "VARIANT", &["ref:orders"]),
                    column("items", "ARRAY(VARCHAR)", &["source:shop.raw_orders"]),
                    column("region", "object", &["seed:countries"]),
                ],
                input_keys: BTreeMap::from([
                    ("ref:orders".to_owned(), keys(&["order_id"])),
                    ("seed:countries".to_owned(), keys(&["country_code"])),
                ]),
                readers: Vec::new(),
                skipped: None,
            }),
            Some(JsonGrain {
                model_keys: keys(&["order_id"]),
                readers: vec!["order_lines_enriched".to_owned(), "unkeyed".to_owned()],
                skipped: Some("no column lineage".to_owned()),
                ..JsonGrain::default()
            }),
            None,
            Some(JsonGrain {
                model_keys: keys(&["id"]),
                skipped: Some("no inferred column types".to_owned()),
                ..JsonGrain::default()
            }),
        ]
    );
}
