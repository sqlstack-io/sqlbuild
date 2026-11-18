//! Facts of a small shop warehouse: two schemas, fingerprints, runs and observations.

use chrono::{DateTime, TimeZone, Utc};

use crate::warehouse_state::models::{
    StateColumn, StateConsumption, StateFingerprint, StateLatestRun, StateLocation,
    StateObservation, StateRelation, SyncedProvenance, WarehouseStateParts,
};

pub(crate) fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

pub(crate) fn at(day: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, day, 12, 0, 0)
        .single()
        .unwrap_or_default()
}

pub(crate) fn location(database: Option<&str>, schema: &str) -> StateLocation {
    StateLocation {
        database: database.map(str::to_owned),
        schema: schema.to_owned(),
    }
}

pub(crate) fn relation(database: Option<&str>, schema: &str, name: &str) -> StateRelation {
    StateRelation {
        database: database.map(str::to_owned),
        schema: schema.to_owned(),
        name: name.to_owned(),
        relation_type: "TABLE".to_owned(),
        columns: vec![StateColumn {
            name: "ID".to_owned(),
            data_type: "NUMBER".to_owned(),
        }],
    }
}

pub(crate) fn fingerprint(
    at_location: StateLocation,
    (node_name, run_id): (&str, &str),
    day: u32,
) -> StateFingerprint {
    StateFingerprint {
        location: at_location.clone(),
        node_type: "model".to_owned(),
        node_name: node_name.to_owned(),
        target_database: at_location.database,
        target_schema: Some(at_location.schema),
        target_name: Some(node_name.to_owned()),
        run_id: run_id.to_owned(),
        definition_hash: format!("definition-{run_id}"),
        version_hash: format!("version-{run_id}"),
        schema_fingerprint: "schema".to_owned(),
        definition: "SELECT 1".to_owned(),
        metadata_json: "{}".to_owned(),
        ts: at(day),
    }
}

pub(crate) fn observation(
    kind: &str,
    (node, run_id): (&str, &str),
    value: &str,
) -> StateObservation {
    StateObservation {
        location: location(Some("WAREHOUSE"), "SHOP"),
        kind: kind.to_owned(),
        node: node.to_owned(),
        cursor_type: None,
        value: value.to_owned(),
        run_id: run_id.to_owned(),
        observed_at: at(3),
    }
}

pub(crate) fn consumption(at_location: StateLocation, execution_id: &str) -> StateConsumption {
    StateConsumption {
        location: at_location,
        execution_id: Some(execution_id.to_owned()),
        phase: Some("succeeded".to_owned()),
        run_id: Some("run-2".to_owned()),
        node_type: Some("model".to_owned()),
        node_name: Some("orders".to_owned()),
        target_database: Some("WAREHOUSE".to_owned()),
        target_schema: Some("SHOP".to_owned()),
        target_name: Some("orders".to_owned()),
        version_hash: Some("version".to_owned()),
        input_count: Some(0),
        input_kind: None,
        input_name: None,
        input_version: None,
        recorded_at: at(2),
    }
}

/// The shop warehouse: `SHOP` holds the models, `archive` an older copy of orders.
pub(crate) fn shop_parts() -> WarehouseStateParts {
    let shop = location(Some("WAREHOUSE"), "SHOP");
    WarehouseStateParts {
        provenance: SyncedProvenance {
            synced_at: Some(at(4)),
            version: Some("7".to_owned()),
        },
        relations: vec![
            relation(Some("WAREHOUSE"), "SHOP", "ORDERS"),
            relation(Some("WAREHOUSE"), "SHOP", "CUSTOMERS"),
            relation(None, "archive", "Orders"),
            relation(Some("WAREHOUSE"), "SHOP", "_sqlbuild_fingerprints"),
            relation(Some("WAREHOUSE"), "SHOP", "_sqlbuild_consumption"),
        ],
        fingerprints: vec![
            fingerprint(shop.clone(), ("orders", "run-1"), 1),
            fingerprint(shop.clone(), ("orders", "run-2"), 2),
            fingerprint(shop.clone(), ("customers", "run-b"), 2),
            fingerprint(shop.clone(), ("customers", "run-a"), 2),
            fingerprint(location(None, "archive"), ("orders", "run-0"), 1),
        ],
        latest_runs: vec![StateLatestRun {
            location: shop.clone(),
            node_type: "model".to_owned(),
            node_name: "orders".to_owned(),
            run_id: "run-2".to_owned(),
        }],
        observations: vec![
            observation("cursor_max", ("orders", "run-1"), "2026-01-01"),
            observation("cursor_max", ("orders", "run-2"), "2026-01-02"),
            observation("cursor_max", ("customers", "run-b"), "2026-01-02"),
            observation(
                "source_columns",
                ("raw_refunds", "run-2"),
                // SHA-256 of `SELECT 1`.
                r#"{"sql_hash": "e004ebd5b5532a4b85984a62f8ad48a81aa3460c1ca07701f386135d72cdecf5", "columns": ["id", "amount"]}"#,
            ),
        ],
        consumption: vec![
            consumption(shop, "execution-1"),
            consumption(location(Some("REPORTING"), "SHOP"), "execution-2"),
        ],
        has_consumption: true,
    }
}
