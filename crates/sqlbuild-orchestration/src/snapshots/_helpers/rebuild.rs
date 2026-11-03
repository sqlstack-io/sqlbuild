//! Restore current publication pointers and jobs without republishing or touching trigger state.

use crate::declarations::main::rebuild_jobs::rebuild_jobs;
use crate::snapshots::models::SnapshotRecord;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, decode_row::decode_row, query::query};
use crate::store::models::{Change, ControlStore, Parameter, TransactionMode};

/// Internal replay owner; current pointers and jobs change, never trigger history.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "awaiting the complete rebuild command")
)]
pub(crate) async fn rebuild_snapshot_projections(store: &ControlStore) -> Result<(), ControlError> {
    let mut transaction = begin(store, TransactionMode::Write).await?;
    let mut cursor = None;
    loop {
        let (after, parameters) = cursor.as_ref().map_or_else(
            || ("", vec![]),
            |(location, target): &(uuid::Uuid, String)| {
                (
                    " AND (s.location_id>$1 OR (s.location_id=$1 AND s.target>$2))",
                    vec![Parameter::Id(*location), Parameter::Text(target.clone())],
                )
            },
        );
        let rows;
        (transaction, rows) = query(transaction,
            &format!("SELECT s.snapshot_id,s.location_id,s.target,s.generation,s.manifest_digest,s.published_at,s.snapshot_schema_version \
             FROM sqb_snapshots s WHERE NOT EXISTS (SELECT 1 FROM sqb_snapshots newer WHERE newer.location_id=s.location_id \
             AND newer.target=s.target AND newer.generation>s.generation){after} ORDER BY s.location_id,s.target LIMIT 100"),
            parameters).await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            if row["snapshot_schema_version"].as_i64() != Some(1) {
                return Err(ControlError::Invalid(
                    "cannot replay an unsupported snapshot schema".into(),
                ));
            }
            let snapshot: SnapshotRecord = decode_row(row)?;
            transaction = rebuild_jobs(transaction, &snapshot).await?;
            (transaction, _) = query(transaction,
                "INSERT INTO sqb_current_snapshots (location_id,target,snapshot_id,since) VALUES ($1,$2,$3,$4) \
                 ON CONFLICT (location_id,target) DO UPDATE SET snapshot_id=excluded.snapshot_id,since=excluded.since",
                vec![Parameter::Id(snapshot.location_id),Parameter::Text(snapshot.target.clone()),
                    Parameter::Id(snapshot.snapshot_id),Parameter::Time(snapshot.published_at)]).await?;
            transaction.changes.insert(Change::Snapshot(
                snapshot.location_id,
                snapshot.target.clone(),
            ));
            cursor = Some((snapshot.location_id, snapshot.target));
        }
    }
    for table in ["sqb_job_nodes", "sqb_jobs"] {
        (transaction, _) = query(transaction,
            &format!("DELETE FROM {table} WHERE NOT EXISTS (SELECT 1 FROM sqb_snapshots s WHERE s.location_id={table}.location_id AND s.target={table}.target)"),
            vec![]).await?;
    }
    commit(transaction).await
}
