//! Samples are immutable hourly estimates, exact-scoped and independent of structural facts.

use chrono::{Duration, Timelike, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::main::{
    ingest_statistics::ingest_statistics, read_person_statistics::read_person_statistics,
};
use crate::warehouse_metadata::models::{MetadataBatch, StatisticsQuery};

pub(crate) async fn statistics_reads(
    store: &ControlStore,
    credentials: (&AuditContext, (&str, &str)),
    (location, other, session_id): (Uuid, Uuid, Uuid),
) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/duckshop/edge_cases/orchestration/warehouse_statistics.json");
    let data: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("fixture"))
        .expect("statistics fixture");
    let now = Utc::now()
        .with_minute(0)
        .expect("minute")
        .with_second(0)
        .expect("second")
        .with_nanosecond(0)
        .expect("nanosecond");
    let transaction = begin(store, TransactionMode::Read)
        .await
        .expect("facts before");
    let (transaction, before) = query(
        transaction,
        "SELECT COUNT(*) AS total FROM recall_events",
        vec![],
    )
    .await
    .expect("fact count");
    commit(transaction).await.expect("facts read");
    let records: Vec<Value> = (0..data["retained_hours"].as_i64().expect("hours"))
        .map(|index| {
            let mut record = data["record"].clone();
            record["observed_at"] = json!(now - Duration::hours(205 - index));
            record
        })
        .collect();
    for records in records.chunks(100) {
        let batch: MetadataBatch = serde_json::from_value(
            json!({"kind":"statistics","session_id":session_id,"generation":1,"records":records}),
        )
        .expect("batch");
        assert!(
            ingest_statistics(store, (location, "development"), &batch)
                .await
                .expect("ingest")
                .applied
        );
        assert!(
            !ingest_statistics(store, (location, "development"), &batch)
                .await
                .expect("lost acknowledgement")
                .applied
        );
    }
    let mut options = StatisticsQuery {
        database: None,
        schema: "main".into(),
        name: "orders_🧾_000".into(),
        after_observed_at: None,
        after_event_id: None,
        limit: Some(u32::MAX),
    };
    let mut actual = Vec::new();
    for expected in [100, 100, 5] {
        let page = read_person_statistics(
            store,
            credentials,
            (location, "development", &options),
            (DefaultRole::None, Default::default()),
        )
        .await
        .expect("page");
        assert_eq!(page.records.len(), expected);
        options.after_observed_at = page.next_after.as_ref().map(|cursor| cursor.observed_at);
        options.after_event_id = page.next_after.map(|cursor| cursor.event_id);
        actual.extend(page.records);
    }
    assert!(options.after_event_id.is_none());
    let actual_times: Vec<_> = actual
        .iter()
        .map(|observation| observation.record.observed_at)
        .collect();
    let expected_times: Vec<_> = (1..=205)
        .rev()
        .map(|hours| now - Duration::hours(hours))
        .collect();
    assert_eq!(actual_times, expected_times);
    assert!(
        actual
            .iter()
            .all(|observation| observation.record.row_count == Some(7)
                && observation.record.byte_count == Some(4096))
    );
    let mut replacement: MetadataBatch = serde_json::from_value(
        json!({"kind":"statistics","session_id":session_id,"generation":1,"records":[records[0]]}),
    )
    .expect("replacement");
    replacement.records[0]["row_count"] = json!(999);
    replacement.records[0]["observed_at"] = json!(
        serde_json::from_value::<chrono::DateTime<chrono::Utc>>(
            replacement.records[0]["observed_at"].clone()
        )
        .expect("observed_at")
            + Duration::seconds(3599)
    );
    assert!(
        !ingest_statistics(store, (location, "development"), &replacement)
            .await
            .expect("same hour first wins")
            .applied
    );
    for (database, schema, name) in [
        (Some("orders_data"), "main", "orders_🧾_000"),
        (None, "landing", "orders_🧾_000"),
        (None, "main", "Orders"),
    ] {
        let mut scoped = replacement.clone();
        scoped.records[0]["state_database"] = json!(database);
        scoped.records[0]["state_schema"] = json!(schema);
        scoped.records[0]["name"] = json!(name);
        assert!(
            ingest_statistics(store, (location, "development"), &scoped)
                .await
                .expect("separate exact identity")
                .applied
        );
        options.database = database.map(String::from);
        options.schema = schema.into();
        options.name = name.into();
        assert_eq!(
            read_person_statistics(
                store,
                credentials,
                (location, "development", &options),
                (DefaultRole::None, Default::default())
            )
            .await
            .expect("exact scope")
            .records
            .len(),
            1
        );
    }
    options.database = None;
    options.schema = "main".into();
    options.name = "orders_🧾_000".into();
    options.limit = Some(1);
    let first = read_person_statistics(
        store,
        credentials,
        (location, "development", &options),
        (DefaultRole::None, Default::default()),
    )
    .await
    .expect("first wins");
    assert_eq!(first.records[0].record.row_count, Some(7));
    for scope in [(other, "development"), (location, "preview")] {
        assert!(matches!(
            read_person_statistics(
                store,
                credentials,
                (scope.0, scope.1, &options),
                (DefaultRole::None, Default::default())
            )
            .await,
            Err(ControlError::Denied { .. })
        ));
    }
    options.after_event_id = Some(Uuid::now_v7());
    assert!(matches!(
        read_person_statistics(
            store,
            credentials,
            (location, "development", &options),
            (DefaultRole::None, Default::default())
        )
        .await,
        Err(ControlError::Invalid(_))
    ));
    for (generation, reader) in [(2, session_id), (1, Uuid::now_v7())] {
        let mut fenced = replacement.clone();
        fenced.generation = generation;
        fenced.session_id = reader;
        assert!(
            ingest_statistics(store, (location, "development"), &fenced)
                .await
                .is_err()
        );
    }
    let mut invalid = replacement.clone();
    invalid.records[0]["byte_count_basis"] = Value::Null;
    assert!(matches!(
        ingest_statistics(store, (location, "development"), &invalid).await,
        Err(ControlError::Invalid(_))
    ));
    let transaction = begin(store, TransactionMode::Read)
        .await
        .expect("facts after");
    let (transaction, after) = query(
        transaction,
        "SELECT COUNT(*) AS total FROM recall_events",
        vec![],
    )
    .await
    .expect("fact count");
    commit(transaction).await.expect("facts read");
    assert_eq!(
        before, after,
        "statistics must not append structural change facts"
    );
    crate::api::tests::statistics_eligibility::statistics_eligibility(
        store,
        credentials,
        (location, session_id),
        &data["record"],
    )
    .await;
    crate::api::tests::statistics_retention::statistics_retention(
        store,
        credentials,
        (location, session_id),
        &data,
    )
    .await;
}
