//! Finished runs past `run_output_days` lose their output: its objects first, then its index; the run and its node logs stay.

use chrono::TimeDelta;
use futures_util::StreamExt;
use object_store::ObjectStoreExt;

use crate::run_output::_helpers::objects::run_prefix;
use crate::run_output::constants::PRUNE_RUNS;
use crate::run_output::models::{ExpiredRun, OutputStore};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::query_records::query_records;
use crate::store::main::write::write;
use crate::store::models::{ControlStore, Parameter};

/// How many runs lost their output; a repeated or interrupted pass finds only output still stored.
pub async fn prune_run_output(
    store: &ControlStore,
    output: &OutputStore,
    days: u16,
) -> Result<usize, ControlError> {
    let expired: Vec<ExpiredRun> = write(store, |transaction| async move {
        let cutoff = transaction.now - TimeDelta::days(i64::from(days));
        query_records(
            transaction,
            "SELECT o.run_id FROM sqb_run_output o JOIN sqb_runs r ON r.run_id=o.run_id \
             WHERE r.ended_at IS NOT NULL AND r.ended_at<$1 ORDER BY r.ended_at LIMIT $2",
            vec![Parameter::Time(cutoff), Parameter::Integer(PRUNE_RUNS)],
        )
        .await
    })
    .await?;
    for ExpiredRun { run_id } in &expired {
        let prefix = run_prefix(output, *run_id);
        let mut listed = output.objects.list(Some(&prefix));
        while let Some(object) = listed.next().await {
            output.objects.delete(&object?.location).await?;
        }
        write(store, |mut transaction| async move {
            for sql in [
                "DELETE FROM sqb_run_output_segments WHERE run_id=$1",
                "DELETE FROM sqb_run_output_chunks WHERE run_id=$1",
                "DELETE FROM sqb_run_output_streams WHERE run_id=$1",
                "DELETE FROM sqb_run_output WHERE run_id=$1",
            ] {
                (transaction, _) = query(transaction, sql, vec![Parameter::Id(*run_id)]).await?;
            }
            Ok((transaction, ()))
        })
        .await?;
    }
    Ok(expired.len())
}
