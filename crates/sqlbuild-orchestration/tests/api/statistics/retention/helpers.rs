//! Bounded global expiration does not depend on catalog or namespace survival.

use chrono::{Duration, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::models::ControlStore;
use crate::warehouse_metadata::_helpers::statistics::prune_statistics;
use crate::warehouse_metadata::main::{
    ingest_statistics::ingest_statistics, read_person_statistics::read_person_statistics,
};
use crate::warehouse_metadata::models::{StatisticsBatch, StatisticsQuery};

pub(crate) async fn statistics_retention(
    store: &ControlStore,
    credentials: (&AuditContext, (&str, &str)),
    (location, session_id): (Uuid, Uuid),
    data: &Value,
) {
    let records: Vec<Value> = (0..data["expired_samples"].as_i64().expect("expired samples"))
        .map(|index| {
            let mut record = data["record"].clone();
            record["state_schema"] = json!("removed_namespace");
            record["name"] = json!(format!("inventory_{index}"));
            record["observed_at"] = json!(Utc::now() - Duration::days(91));
            record
        })
        .collect();
    for records in records.chunks(100) {
        let batch: StatisticsBatch = serde_json::from_value(
            json!({"session_id":session_id,"generation":1,"records":records}),
        )
        .expect("expired batch");
        assert!(
            ingest_statistics(store, (location, "development"), &batch)
                .await
                .expect("expired sample")
                .applied
        );
    }
    let options = StatisticsQuery {
        database: None,
        schema: "removed_namespace".into(),
        name: "inventory_0".into(),
        after_observed_at: None,
        after_event_id: None,
        limit: None,
    };
    assert!(
        read_person_statistics(
            store,
            credentials,
            (location, "development", &options),
            DefaultRole::None
        )
        .await
        .expect("expired rows hidden before cleanup")
        .records
        .is_empty()
    );
    for expected in [100, 100, 5, 0] {
        assert_eq!(
            prune_statistics(store)
                .await
                .expect("global bounded expiration"),
            expected
        );
    }
    let options = StatisticsQuery {
        schema: "main".into(),
        name: "orders_🧾_000".into(),
        ..options
    };
    assert_eq!(
        read_person_statistics(
            store,
            credentials,
            (location, "development", &options),
            DefaultRole::None
        )
        .await
        .expect("retained points survive")
        .records
        .len(),
        100
    );
}
