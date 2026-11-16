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
    serde_json::json!({"name": name, "action": action, "reason": reason, "query_diff": ["+SELECT 1"]})
}

/// A shop plan: orders changed, customers and payments routine, refunds skipped, events incremental.
pub(super) fn shop_plan() -> serde_json::Value {
    serde_json::json!({
        "selected_count": 5,
        "models": [
            planned_model("orders", ("create_table", "query_changed")),
            planned_model("customers", ("create_view", "no_change")),
            planned_model("payments", ("create_view", "no_change")),
            planned_model("refunds", ("skip", "no_change")),
            planned_model("events", ("incremental_merge", "normal_incremental")),
        ],
        "warnings": [],
        "metadata": {"orders": {"columns": 3}},
        "changed": null,
    })
}

/// The shop plan under `--changed`: orders, customers and a seed run; payments and events are unchanged.
pub(super) fn changed_shop_plan() -> serde_json::Value {
    let mut plan = shop_plan();
    plan["changed"] = serde_json::json!({
        "counts": ["definition"],
        "reach": "all",
        "runs": [
            {"node": "model:orders", "reason": "definition changed", "downstream_of": null},
            {"node": "model:customers", "reason": "upstream changed", "downstream_of": "model:orders"},
            {"node": "seed:countries", "reason": "no completed record", "downstream_of": null},
        ],
        "held_back": [{"node": "model:daily_revenue", "refreshed_by": ["finance"]}],
        "unchanged": ["model:payments", "model:events"],
    });
    plan
}
