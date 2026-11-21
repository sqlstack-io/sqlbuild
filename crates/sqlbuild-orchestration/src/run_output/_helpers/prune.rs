//! Removing one run's output so the index never points at a missing object.

use chrono::TimeDelta;
use futures_util::StreamExt;
use object_store::ObjectStoreExt;
use uuid::Uuid;

use crate::run_output::_helpers::objects::run_prefix;
use crate::run_output::constants::PRUNE_RUNS;
use crate::run_output::models::{ExpiredRun, OutputStore};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::query_records::query_records;
use crate::store::main::write::write;
use crate::store::models::{ControlStore, Parameter};

/// Index first (all but the `sqb_run_output` row), then objects, then that row, so a failure is retried and nothing points at a deleted object.
pub(crate) async fn prune_run(
    store: &ControlStore,
    output: &OutputStore,
    run_id: Uuid,
) -> Result<(), ControlError> {
    delete_index(
        store,
        run_id,
        &[
            "DELETE FROM sqb_run_output_marks WHERE run_id=$1",
            "DELETE FROM sqb_run_output_segments WHERE run_id=$1",
            "DELETE FROM sqb_run_output_chunks WHERE run_id=$1",
            "DELETE FROM sqb_run_output_streams WHERE run_id=$1",
        ],
    )
    .await?;
    let prefix = run_prefix(output, run_id);
    let mut listed = output.objects.list(Some(&prefix));
    while let Some(object) = listed.next().await {
        output.objects.delete(&object?.location).await?;
    }
    delete_index(
        store,
        run_id,
        &["DELETE FROM sqb_run_output WHERE run_id=$1"],
    )
    .await
}

async fn delete_index(
    store: &ControlStore,
    run_id: Uuid,
    statements: &[&str],
) -> Result<(), ControlError> {
    write(store, |mut transaction| async move {
        for sql in statements {
            (transaction, _) = query(transaction, sql, vec![Parameter::Id(run_id)]).await?;
        }
        Ok((transaction, ()))
    })
    .await
}

/// Runs that ended more than `days` ago and still have output, oldest first.
pub(crate) async fn expired_runs(
    store: &ControlStore,
    days: u16,
) -> Result<Vec<ExpiredRun>, ControlError> {
    write(store, |transaction| async move {
        let cutoff = transaction.now - TimeDelta::days(i64::from(days));
        query_records(
            transaction,
            "SELECT o.run_id FROM sqb_run_output o JOIN sqb_runs r ON r.run_id=o.run_id \
             WHERE r.ended_at IS NOT NULL AND r.ended_at<$1 ORDER BY r.ended_at LIMIT $2",
            vec![Parameter::Time(cutoff), Parameter::Integer(PRUNE_RUNS)],
        )
        .await
    })
    .await
}

/// Tell the server's operator that output retention failed, for one run or for the pass; it is retried next pass.
pub(crate) fn report_retention_failure(run_id: Option<Uuid>, error: &ControlError) {
    match run_id {
        Some(run_id) => eprintln!(
            "Run output retention: could not remove the output of run {run_id}; retrying next pass: {error}"
        ),
        None => eprintln!("Run output retention failed; retrying next pass: {error}"),
    }
}
