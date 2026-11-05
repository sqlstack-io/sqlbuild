//! Partial namespace sampling cannot hide unsampled relations after a reader restart.

use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::models::ControlStore;
use crate::warehouse_metadata::main::{
    ingest_statistics::ingest_statistics, read_person_statistics::read_person_statistics,
    read_statistics_due::read_statistics_due,
};
use crate::warehouse_metadata::models::{
    MetadataBatch, StatisticsDueRead, StatisticsQuery, WarehouseStateLocation,
};

pub(crate) async fn statistics_eligibility(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location, session_id): (Uuid, Uuid),
    record: &Value,
) {
    let mut request = StatisticsDueRead {
        session_id,
        generation: 1,
        location: WarehouseStateLocation {
            database: None,
            schema: "main".into(),
        },
        after: None,
        limit: 100,
    };
    let first = read_statistics_due(store, (location, "development"), &request)
        .await
        .expect("due first page");
    assert_eq!(first.names.len(), 100);
    let mut sample = record.clone();
    sample["name"] = json!(first.names[0]);
    sample["observed_at"] = json!(Utc::now());
    sample["row_count"] = Value::Null;
    sample["byte_count"] = json!(0);
    let batch: MetadataBatch = serde_json::from_value(
        json!({"kind":"statistics","session_id":session_id,"generation":1,"records":[sample]}),
    )
    .expect("one sampled relation");
    assert!(
        ingest_statistics(store, (location, "development"), &batch)
            .await
            .expect("partial sampling")
            .applied
    );
    let options = StatisticsQuery {
        database: None,
        schema: "main".into(),
        name: first.names[0].clone(),
        after_observed_at: Some(
            serde_json::from_value(batch.records[0]["observed_at"].clone()).expect("observed_at"),
        ),
        after_event_id: Some(Uuid::nil()),
        limit: None,
    };
    let observed = read_person_statistics(
        store,
        credentials,
        (location, "development", &options),
        (DefaultRole::None, Default::default()),
    )
    .await
    .expect("unknown rows and zero bytes remain distinct");
    let current = observed.records.last().expect("current-hour sample");
    assert_eq!(current.record.row_count, None);
    assert_eq!(current.record.byte_count, Some(0));
    assert_eq!(
        current.record.byte_count_basis.as_deref(),
        Some("synthetic_catalog_bytes")
    );
    let remaining = read_statistics_due(store, (location, "development"), &request)
        .await
        .expect("restart from committed per-relation progress");
    assert_eq!(remaining.names, first.names[1..]);
    assert_eq!(remaining.next_after, first.next_after);
    request.after = remaining.next_after;
    let second = read_statistics_due(store, (location, "development"), &request)
        .await
        .expect("next page");
    assert_eq!(second.names.len(), 100);
    request.after = second.next_after;
    let last = read_statistics_due(store, (location, "development"), &request)
        .await
        .expect("last page");
    assert_eq!(last.names.len(), 5);
    assert!(last.next_after.is_none());
    request.after = None;
    request.location.database = Some("orders_data".into());
    assert_eq!(
        read_statistics_due(store, (location, "development"), &request)
            .await
            .expect("exact database")
            .names,
        ["orders_🧾_000"]
    );
    request.generation = 2;
    assert!(
        read_statistics_due(store, (location, "development"), &request)
            .await
            .is_err()
    );
}
