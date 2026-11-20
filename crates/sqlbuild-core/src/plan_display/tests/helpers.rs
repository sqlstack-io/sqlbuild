use crate::plan_display::models::{ModelLabelFacts, TimestampGrainFacts};

/// A planned model with only its materialization and action.
pub(super) fn facts(materialization_type: &str, action: &str) -> ModelLabelFacts {
    ModelLabelFacts {
        materialization_type: materialization_type.to_owned(),
        action: action.to_owned(),
        ..ModelLabelFacts::default()
    }
}

/// A timestamp cursor with the strategy, declared grain and input grains given.
pub(super) fn timestamp_grains(
    microbatch_strategy: Option<&str>,
    downstream_grain: Option<&str>,
    input_grains: &[Option<&str>],
) -> TimestampGrainFacts {
    TimestampGrainFacts {
        cursor_type: Some("timestamp".to_owned()),
        microbatch_strategy: microbatch_strategy.map(str::to_owned),
        downstream_grain: downstream_grain.map(str::to_owned),
        input_grains: input_grains
            .iter()
            .map(|grain| grain.map(str::to_owned))
            .collect(),
    }
}

fn planned_model(name: &str, (action, reason): (&str, &str)) -> serde_json::Value {
    serde_json::json!({"name": name, "materialization_type": "table", "action": action, "reason": reason})
}

/// A shop plan under `--changed` with a row in every section, from changed models to warnings.
pub(super) fn full_plan() -> serde_json::Value {
    let mut orders = planned_model("orders", ("create_table", "query_changed"));
    orders["query_diff"] =
        serde_json::json!(["--- a", "+++ b", "@@ -1 +1 @@", "-select  id", "+select id"]);
    let mut payments = planned_model("payments", ("create_table", "query_changed"));
    payments["query_diff"] = serde_json::json!(["-select id", "+SELECT id"]);
    payments["config_diff"] = serde_json::json!(["-  \"tags\": []", "+  \"tags\": [\"finance\"]"]);
    let mut old_orders = planned_model("old_orders", ("create_table", "renamed"));
    old_orders["origin_model"] = serde_json::json!("orders_v1");
    old_orders["old_name"] = serde_json::json!({"action": "view_only", "view": "main.orders_v1", "expires_at": "2026-12-18T00:00:00Z"});
    let mut totals = planned_model("totals", ("create_table", "config_changed"));
    totals["config_diff"] = serde_json::json!([
        "--- a",
        "+++ b",
        "@@ -1,2 +1,2 @@",
        "   \"materialized\": \"table\",",
        "-  \"full_refresh\": true,",
        "+  \"full_refresh\": false,"
    ]);
    let mut daily_orders = planned_model("daily_orders", ("create_table", "first_run"));
    daily_orders["config_diff"] = serde_json::json!([
        "--- a",
        "+++ b",
        "@@ -2,3 +2,3 @@",
        "   \"meta\": {",
        "-    \"owner\": \"sales\"",
        "+    \"owner\": \"finance\"",
        "   },"
    ]);
    let mut daily_payments = planned_model("daily_payments", ("create_table", "first_run"));
    daily_payments["config_diff"] = serde_json::json!([
        "--- a",
        "+++ b",
        "@@ -1,3 +1,4 @@",
        "-  \"materialized\": \"table\"",
        "+  \"materialized\": \"table\",",
        "+  \"tags\": \"none\""
    ]);
    let mut ledger = planned_model("ledger", ("create_table", "schema_changed"));
    ledger["config_diff"] = serde_json::json!([
        "-  \"grain\": \"hour\",",
        "+  \"grain\": \"day\",",
        "-  \"lookback\": 2,",
        "+  \"lookback\": 3,",
        "-  \"partition_by\": \"event_date\"",
        "+  \"partition_by\": \"event_hour\""
    ]);
    serde_json::json!({
        "selected_count": 12,
        "models": [
            orders,
            payments,
            planned_model("refunds", ("create_table", "function_changed")),
            totals,
            ledger,
            daily_orders,
            daily_payments,
            old_orders,
            planned_model("archive", ("create_table", "full_refresh")),
            planned_model("customers", ("create_view", "no_change")),
            planned_model("Customer_Notes", ("create_view", "no_change")),
            planned_model("events", ("incremental_merge", "normal_incremental")),
            planned_model("skipped_model", ("skip", "no_change")),
            planned_model("not_run", ("create_view", "no_change")),
        ],
        "seeds": [
            {"name": "countries", "reason": "first_run"},
            {"name": "currency_rates", "reason": "config_changed"},
            {"name": "regions", "reason": "no_change"},
            {"name": "idle_seed", "reason": "no_change"},
        ],
        "source_loads": [
            {"name": "raw_orders", "kind": "loader", "is_reload": true, "write_strategy": "replace"},
            {"name": "raw_payments", "kind": "loader", "is_reload": false, "write_strategy": "append"},
            {"name": "shop_db", "kind": "source", "is_reload": true},
            {"name": "partner_feed", "kind": "source", "is_reload": false, "cursor_column": "updated_at"},
        ],
        "functions": [
            {"name": "to_cents", "language": "sql", "reason": "definition_changed"},
            {"name": "clean_name", "language": "sql", "reason": "no_change"},
        ],
        "python_nodes": [{"name": "score_orders", "kind": "asset", "phase": "after", "identity_status": "changed"}],
        "warnings": [
            {"severity": "warning", "model_name": "orders", "message": "orders: already-built dates keep their old results"},
            {"severity": "warning", "message": "scenario 'checkout' step 'pay' is stale", "code": "W1"},
            {"severity": "warning", "message": "model stg__orders_2 has no tests."},
            {"severity": "warning", "message": "scenario 'checkout' step 'ship' is stale", "code": "W1"},
            {"severity": "warning", "message": "model stg__customers_3 has no tests."},
            {"severity": "warning", "message": "scenario 'returns' step `refund` is stale", "code": "W1"},
        ],
        "migrations": [
            {"model": "old_orders", "origin_model": "orders_v1", "decision": "renamed", "completed_at": null, "is_rename": true},
            {"model": "totals", "origin_model": "totals_legacy", "decision": "forced_replace", "completed_at": null, "is_rename": false},
        ],
        "column_migrations": [{"model": "ledger", "origin_column": "amt", "destination_column": "amount", "decision": "rename"}],
        "table_type_conversions": [{"model": "totals", "actual_type": "view", "desired_type": "table"}],
        "retention": [{"scope": "table", "name": "main.events", "models": ["events"], "desired_days": 7, "actual_days": 1, "direction": "increase"}],
        "metadata": {"orders": {"columns": 3}},
        "synced_copy": {"synced_at": null, "cursor_window_starts": {"events": "2026-11-01"}, "unknown_until_run": []},
        "changed": {
            "counts": ["definition"],
            "reach": "all",
            "runs": [
                {"node": "model:orders", "reason": "definition changed", "downstream_of": null},
                {"node": "model:payments", "reason": "definition changed", "downstream_of": null},
                {"node": "model:refunds", "reason": "definition changed", "downstream_of": null},
                {"node": "model:totals", "reason": "definition changed", "downstream_of": null},
                {"node": "model:ledger", "reason": "definition changed", "downstream_of": null},
                {"node": "model:daily_orders", "reason": "no completed record", "downstream_of": null},
                {"node": "model:daily_payments", "reason": "no completed record", "downstream_of": null},
                {"node": "model:old_orders", "reason": "definition changed", "downstream_of": null},
                {"node": "model:archive", "reason": "definition changed", "downstream_of": null},
                {"node": "model:customers", "reason": "upstream changed", "downstream_of": "model:orders"},
                {"node": "model:Customer_Notes", "reason": "no completed record", "downstream_of": null},
                {"node": "model:events", "reason": "no completed record", "downstream_of": null},
                {"node": "seed:countries", "reason": "no completed record", "downstream_of": null},
                {"node": "seed:currency_rates", "reason": "definition changed", "downstream_of": null},
                {"node": "seed:regions", "reason": "no completed record", "downstream_of": null},
            ],
            "held_back": [
                {"node": "model:daily_revenue", "refreshed_by": ["finance"]},
                {"node": "model:revenue_report", "reads": "model:daily_revenue"},
            ],
            "unchanged": ["model:not_run", "seed:idle_seed"],
        },
    })
}

/// A first build of `count` table models, as large projects plan it; `changed` runs each under `--changed` with a warning of its own.
pub(super) fn first_build(count: usize, changed: bool) -> serde_json::Value {
    let models: Vec<serde_json::Value> = (0..count)
        .map(|index| {
            let mut model =
                planned_model(&format!("model_{index:05}"), ("create_table", "first_run"));
            model["qualified_name"] =
                serde_json::json!(format!("analytics.marts.model_{index:05}"));
            model["backfill"] = serde_json::json!({"action": "full", "duration": null});
            model
        })
        .collect();
    let names = (0..count).map(|index| format!("model_{index:05}"));
    let runs: Vec<serde_json::Value> = names
        .clone()
        .map(|name| serde_json::json!({"node": format!("model:{name}"), "reason": "no completed record", "downstream_of": null}))
        .take(count * usize::from(changed))
        .collect();
    let warnings: Vec<serde_json::Value> = (0..count * usize::from(changed))
        .map(|index| serde_json::json!({"severity": "warning", "message": format!("unusual setting {} on this model", word(index))}))
        .collect();
    let decision = [
        serde_json::Value::Null,
        serde_json::json!({"counts": ["definition"], "reach": "all", "runs": runs, "held_back": [], "unchanged": []}),
    ][usize::from(changed)]
    .clone();
    serde_json::json!({
        "selected_count": count, "models": models, "seeds": [], "source_loads": [], "functions": [],
        "python_nodes": [], "warnings": warnings, "migrations": [], "column_migrations": [],
        "table_type_conversions": [], "retention": [], "synced_copy": null,
        "changed": decision,
    })
}

/// A distinct three-letter word for each index below 17,576, so no warning template elides it.
fn word(index: usize) -> String {
    (0..3)
        .map(|place: u32| {
            char::from(b'a' + u8::try_from(index / 26_usize.pow(place) % 26).unwrap_or_default())
        })
        .collect()
}

/// A row's name: a model's or migration's, a node key's name, or a warning's message.
pub(super) fn row_label(row: &serde_json::Value) -> String {
    let key = ["name", "model", "node", "message"]
        .iter()
        .find_map(|field| row.get(*field).and_then(serde_json::Value::as_str))
        .or_else(|| row.as_str())
        .unwrap_or_default();
    key.split_once(':')
        .filter(|(kind, _)| ["model", "seed"].contains(kind))
        .map_or(key, |(_, name)| name)
        .to_owned()
}

/// Each row's name, in order.
pub(super) fn labels(rows: &[serde_json::Value]) -> Vec<String> {
    rows.iter().map(row_label).collect()
}

/// Owned copies of names.
pub(super) fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}
