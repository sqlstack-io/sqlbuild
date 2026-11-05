use uuid::Uuid;

use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) async fn location_state(store: &ControlStore) -> (i64, i64) {
    let sql = "SELECT (SELECT COUNT(*) FROM sqb_locations), \
               (SELECT COUNT(*) FROM sqb_security_audit WHERE action='location.created')";
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!()
        };
        return rusqlite::Connection::open(path)
            .expect("inspect locations")
            .query_row(sql, [], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("location and audit counts");
    };
    sqlx_core::query_as::query_as(sql)
        .fetch_one(pool)
        .await
        .expect("location and audit counts")
}

pub(crate) async fn stored_hashes(store: &ControlStore) -> Vec<String> {
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!()
        };
        let connection = rusqlite::Connection::open(path).expect("inspect durable tokens");
        return connection
            .prepare("SELECT secret_hash FROM sqb_access_tokens")
            .expect("token hashes")
            .query_map([], |row| row.get(0))
            .expect("read token hashes")
            .collect::<Result<_, _>>()
            .expect("hashes");
    };
    sqlx_core::query_scalar::query_scalar("SELECT secret_hash FROM sqb_access_tokens")
        .fetch_all(pool)
        .await
        .expect("read durable token hashes")
}

/// Committed security facts; every authority change writes exactly one in its own transaction.
pub(crate) async fn security_event_count(
    store: &ControlStore,
) -> Result<u64, crate::store::errors::ControlError> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await?,
        "SELECT COUNT(*) AS revision FROM sqb_security_audit",
        vec![],
    )
    .await?;
    commit(transaction).await?;
    rows.first()
        .and_then(|row| row.get("revision"))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            crate::store::errors::ControlError::Schema("security event count is invalid".into())
        })
}

/// Test-only clock skew: every audit row becomes older than the given number of days.
pub(crate) async fn age_audit(store: &ControlStore, days: i64) {
    let transaction = begin(store, TransactionMode::Write)
        .await
        .expect("age audit");
    let then = transaction.now - chrono::Duration::days(days);
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_security_audit SET occurred_at=$1",
        vec![Parameter::Time(then)],
    )
    .await
    .expect("aged audit rows");
    commit(transaction).await.expect("commit aged audit");
}

/// Acknowledge every audit row up to and including `through` for one sink, in storage order.
pub(crate) async fn acknowledge_audit(store: &ControlStore, sink_id: &str, through: i64) {
    let transaction = begin(store, TransactionMode::Write)
        .await
        .expect("acknowledge audit");
    let now = transaction.now;
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_security_audit_deliveries (sink_id, storage_id, event_id, delivered_at) \
         SELECT $1, storage_id, event_id, $2 FROM sqb_security_audit WHERE storage_id<=$3",
        vec![
            Parameter::Text(sink_id.into()),
            Parameter::Time(now),
            Parameter::Integer(through),
        ],
    )
    .await
    .expect("delivery rows");
    commit(transaction).await.expect("commit deliveries");
}

/// Remaining audit event IDs with their storage IDs, and the delivery rows that remain.
pub(crate) async fn audit_rows(store: &ControlStore) -> (Vec<(Uuid, i64)>, i64) {
    let transaction = begin(store, TransactionMode::Read)
        .await
        .expect("read audit");
    let (transaction, rows) = query(
        transaction,
        "SELECT event_id, storage_id FROM sqb_security_audit ORDER BY storage_id",
        vec![],
    )
    .await
    .expect("audit rows");
    let (transaction, deliveries) = query(
        transaction,
        "SELECT COUNT(*) AS deliveries FROM sqb_security_audit_deliveries",
        vec![],
    )
    .await
    .expect("delivery count");
    commit(transaction).await.expect("commit audit read");
    (
        rows.iter()
            .map(|row| {
                (
                    serde_json::from_value(row["event_id"].clone()).expect("event ID"),
                    row["storage_id"].as_i64().expect("storage ID"),
                )
            })
            .collect(),
        deliveries[0]["deliveries"].as_i64().expect("deliveries"),
    )
}
