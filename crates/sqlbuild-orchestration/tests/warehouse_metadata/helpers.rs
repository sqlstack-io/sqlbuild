//! A small served copy and the state fixture that holds the same facts.

use serde_json::{Value, json};

/// A copy as the server serves it: one relation, a fingerprint, a run, an observation and a consumption row.
pub(crate) fn served_copy() -> Value {
    json!({
        "synced_at": "2026-03-01T12:00:00Z",
        "relations": [{"state_database": "WAREHOUSE", "state_schema": "SHOP", "name": "ORDERS",
            "relation_type": "TABLE", "columns": [{"name": "ID", "data_type": "NUMBER"}]}],
        "fingerprints": [{"event_id": "fingerprint-1", "state_database": "WAREHOUSE", "state_schema": "SHOP",
            "node_type": "model", "node_name": "orders", "target_database": "WAREHOUSE",
            "target_schema": "SHOP", "target_name": "ORDERS", "run_id": "run-1",
            "definition_hash": "definition", "version_hash": "version", "schema_fingerprint": "id:NUMBER",
            "definition": "SELECT 1 AS id", "metadata_json": "{}", "ts": "2026-03-01T10:00:00Z"}],
        "latest_runs": [{"state_database": "WAREHOUSE", "state_schema": "SHOP", "node_type": "model",
            "node_name": "orders", "run_id": "run-1"}],
        "observations": [{"state_database": "WAREHOUSE", "state_schema": "SHOP",
            "observation_id": "observation-1", "kind": "cursor_max", "node": "orders",
            "cursor_type": "integer", "value": "41", "run_id": "run-1",
            "observed_at": "2026-03-01T10:00:01Z"}],
        "consumption": [{"state_database": "WAREHOUSE", "state_schema": "SHOP", "execution_id": "execution-1",
            "phase": "succeeded", "run_id": "run-1", "node_type": "model", "node_name": "orders",
            "target_database": "WAREHOUSE", "target_schema": "SHOP", "target_name": "ORDERS",
            "version_hash": "version", "input_count": 0, "input_kind": null, "input_name": null,
            "input_version": null, "recorded_at": "2026-03-01T10:00:00Z"}],
        "version": "mirror-7",
    })
}

/// The same facts as a stored state fixture holds them.
pub(crate) fn stored_parts() -> Value {
    let location = json!({"database": "WAREHOUSE", "schema": "SHOP"});
    json!({
        "provenance": {"synced_at": "2026-03-01T12:00:00Z", "version": "mirror-7"},
        "relations": [{"database": "WAREHOUSE", "schema": "SHOP", "name": "ORDERS", "relation_type": "TABLE",
            "columns": [{"name": "ID", "data_type": "NUMBER"}]}],
        "fingerprints": [{"location": location, "node_type": "model", "node_name": "orders",
            "target_database": "WAREHOUSE", "target_schema": "SHOP", "target_name": "ORDERS",
            "run_id": "run-1", "definition_hash": "definition", "version_hash": "version",
            "schema_fingerprint": "id:NUMBER", "definition": "SELECT 1 AS id", "metadata_json": "{}",
            "ts": "2026-03-01T10:00:00Z"}],
        "latest_runs": [{"location": location, "node_type": "model", "node_name": "orders", "run_id": "run-1"}],
        "observations": [{"location": location, "kind": "cursor_max", "node": "orders",
            "cursor_type": "integer", "value": "41", "run_id": "run-1",
            "observed_at": "2026-03-01T10:00:01Z"}],
        "consumption": [{"location": location, "execution_id": "execution-1", "phase": "succeeded",
            "run_id": "run-1", "node_type": "model", "node_name": "orders",
            "target_database": "WAREHOUSE", "target_schema": "SHOP", "target_name": "ORDERS",
            "version_hash": "version", "input_count": 0, "input_kind": null, "input_name": null,
            "input_version": null, "recorded_at": "2026-03-01T10:00:00Z"}],
        "has_consumption": true,
    })
}
