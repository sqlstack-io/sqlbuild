//! A finished run's worker log goes whole once it is too old or too many newer runs finished; the run, its nodes and the scheduler's node history stay.

use std::collections::BTreeSet;

use sqlbuild_recall::schema::constants::INVOCATION_TABLES;
use sqlbuild_recall::schema::main::table_spec::table_spec;

use crate::runs::constants::{RUN_LOG_DIGEST_CHUNK, RUN_LOG_RETENTION_BATCH};
use crate::runs::models::RunLogRetention;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};

/// Claims of finished runs past either limit whose worker log is still stored, oldest first. Worker invocations are `worker/<claim>` and `worker/<claim>/<session>`; the stored logs, not every run ever finished, bound the join.
const EXPIRED_CLAIMS: &str = "WITH logged AS (\
    SELECT DISTINCT substr(invocation_id, 8, 36) AS claim FROM recall_invocations WHERE invocation_id LIKE 'worker/%' \
    UNION SELECT DISTINCT substr(invocation_id, 8, 36) AS claim FROM recall_output WHERE invocation_id LIKE 'worker/%'), \
    ranked AS (SELECT worker_instance, ended_at, ROW_NUMBER() OVER (PARTITION BY location_id, target \
    ORDER BY ended_at DESC, run_id DESC) AS newest FROM sqb_runs \
    WHERE ended_at IS NOT NULL AND status IN ('success', 'warn', 'partial', 'failed', 'cancelled')) \
    SELECT logged.claim FROM ranked JOIN logged ON logged.claim = CAST(ranked.worker_instance AS TEXT) \
    WHERE ranked.newest > $1 OR ranked.ended_at < $2 ORDER BY ranked.ended_at LIMIT $3";

/// Batches commit independently; a repeated or interrupted pass finds only logs still stored.
pub(crate) async fn prune_run_logs(
    store: &ControlStore,
    retention: RunLogRetention,
) -> Result<usize, ControlError> {
    let mut deleted = 0;
    loop {
        let transaction = begin(store, TransactionMode::Write).await?;
        let now = transaction.now;
        let (transaction, rows) = query(
            transaction,
            EXPIRED_CLAIMS,
            vec![
                Parameter::Integer(i64::from(retention.count())),
                Parameter::Time(retention.cutoff(now)),
                Parameter::Integer(RUN_LOG_RETENTION_BATCH as i64),
            ],
        )
        .await?;
        let claims: Vec<String> = rows
            .iter()
            .filter_map(|row| row["claim"].as_str().map(str::to_owned))
            .collect();
        let transaction = delete_logs(transaction, &claims).await?;
        commit(transaction).await?;
        deleted += claims.len();
        if claims.len() < RUN_LOG_RETENTION_BATCH {
            return Ok(deleted);
        }
    }
}

/// Every Recall row of the claims' invocations, then stored artifacts no remaining reference names. `worker/<claim>/<session>` sorts between `worker/<claim>/` and `worker/<claim>0`, so a key range finds nested invocations by index.
async fn delete_logs(
    mut transaction: ControlTransaction,
    claims: &[String],
) -> Result<ControlTransaction, ControlError> {
    let mut digests: BTreeSet<String> = BTreeSet::new();
    for claim in claims {
        let namespace = format!("worker/{claim}");
        let bounds = vec![
            Parameter::Text(namespace.clone()),
            Parameter::Text(format!("{namespace}/")),
            Parameter::Text(format!("{namespace}0")),
        ];
        let owned = "(invocation_id=$1 OR (invocation_id>$2 AND invocation_id<$3))";
        let rows;
        (transaction, rows) = query(
            transaction,
            &format!("SELECT DISTINCT digest FROM recall_artifact_refs WHERE {owned}"),
            bounds.clone(),
        )
        .await?;
        digests.extend(
            rows.iter()
                .filter_map(|row| row["digest"].as_str().map(str::to_owned)),
        );
        for table in INVOCATION_TABLES {
            (transaction, _) = query(
                transaction,
                &format!("DELETE FROM {} WHERE {owned}", table_spec(table).name),
                bounds.clone(),
            )
            .await?;
        }
    }
    let digests: Vec<String> = digests.into_iter().collect();
    for chunk in digests.chunks(RUN_LOG_DIGEST_CHUNK) {
        (transaction, _) = query(
            transaction,
            &format!(
                "DELETE FROM recall_artifacts WHERE digest IN ({}) AND NOT EXISTS \
                 (SELECT 1 FROM recall_artifact_refs AS ref WHERE ref.digest = recall_artifacts.digest)",
                Parameter::placeholders(1..=chunk.len())
            ),
            chunk.iter().cloned().map(Parameter::Text).collect(),
        )
        .await?;
    }
    Ok(transaction)
}
