//! Bounded global expiration does not depend on catalog or namespace survival.

use chrono::{Duration, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::models::ControlStore;
use crate::warehouse_metadata::main::ingest::prune_statistics::prune_statistics;
use crate::warehouse_metadata::main::{
    ingest::ingest_statistics::ingest_statistics,
    person_reads::read_person_statistics::read_person_statistics,
};
use crate::warehouse_metadata::models::{MetadataBatch, StatisticsQuery, StatisticsRetention};

pub(crate) async fn statistics_retention(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
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
        let batch: MetadataBatch = serde_json::from_value(
            json!({"kind":"statistics","session_id":session_id,"generation":1,"records":records}),
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
            (DefaultRole::None, Default::default())
        )
        .await
        .expect("expired rows hidden before cleanup")
        .records
        .is_empty()
    );
    for expected in [100, 100, 5, 0] {
        assert_eq!(
            prune_statistics(store, StatisticsRetention::default())
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
            (DefaultRole::None, Default::default())
        )
        .await
        .expect("retained points survive")
        .records
        .len(),
        100
    );
    configured_retention(store, credentials, (location, session_id), data).await;
}

/// A shorter installation horizon hides and then deletes samples the default would keep.
async fn configured_retention(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location, session_id): (Uuid, Uuid),
    data: &Value,
) {
    assert!(StatisticsRetention::days(0).is_err());
    let horizon = StatisticsRetention::days(5).expect("five-day horizon");
    let records: Vec<Value> = [10, 2]
        .into_iter()
        .map(|age| {
            let mut record = data["record"].clone();
            record["name"] = json!("customers_horizon");
            record["observed_at"] = json!(Utc::now() - Duration::days(age));
            record
        })
        .collect();
    let batch: MetadataBatch = serde_json::from_value(
        json!({"kind":"statistics","session_id":session_id,"generation":1,"records":records}),
    )
    .expect("horizon batch");
    assert!(
        ingest_statistics(store, (location, "development"), &batch)
            .await
            .expect("horizon samples")
            .applied
    );
    let options = StatisticsQuery {
        database: None,
        schema: "main".into(),
        name: "customers_horizon".into(),
        after_observed_at: None,
        after_event_id: None,
        limit: None,
    };
    let visible = async |retention| {
        read_person_statistics(
            store,
            credentials,
            (location, "development", &options),
            (DefaultRole::None, retention),
        )
        .await
        .expect("horizon read")
        .records
        .len()
    };
    assert_eq!(visible(StatisticsRetention::default()).await, 2);
    assert_eq!(visible(horizon).await, 1);
    assert_eq!(
        prune_statistics(store, StatisticsRetention::default())
            .await
            .expect("default keeps both"),
        0
    );
    let pruned = prune_statistics(store, horizon)
        .await
        .expect("configured expiration");
    assert!(
        (2..100).contains(&pruned),
        "the shorter horizon expires both the old sample and older retained hours"
    );
    assert_eq!(
        prune_statistics(store, horizon)
            .await
            .expect("configured expiration completes"),
        0
    );
    assert_eq!(visible(StatisticsRetention::default()).await, 1);
}
