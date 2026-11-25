//! A failed run's Recall facts for its `run_failed` event: audit failures, the failing invocation's error and the phases it reported.

use uuid::Uuid;

use crate::runs::constants::SUPERSEDED_CHILD;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_history::_helpers::run_failures::read_member_failure_recall;
use crate::warehouse_history::models::RunFailureRecall;

/// A failed run's audit failures, failing invocation and reported phases, over the run and its current group members.
pub(crate) async fn read_run_failure_recall(
    transaction: ControlTransaction,
    (run_id, limit): (Uuid, usize),
) -> Result<(ControlTransaction, RunFailureRecall), ControlError> {
    let (mut transaction, runs) = query(
        transaction,
        &format!(
            "SELECT r.run_id FROM sqb_runs r \
         WHERE r.run_id=$1 OR (r.parent_run_id=$1 AND NOT {SUPERSEDED_CHILD}) ORDER BY r.run_id"
        ),
        vec![Parameter::Id(run_id)],
    )
    .await?;
    let mut recall = RunFailureRecall::default();
    for run in runs {
        let member = Uuid::parse_str(run["run_id"].as_str().unwrap_or_default())
            .map_err(|error| ControlError::Schema(format!("stored run ID: {error}")))?;
        let found;
        (transaction, found) = read_member_failure_recall(transaction, member).await?;
        recall.audit_count += found.audit_count;
        recall.audits.extend(found.audits);
        recall.phases.extend(found.phases);
        recall.invocation_error = recall.invocation_error.or(found.invocation_error);
    }
    recall
        .audits
        .sort_by(|left, right| left.name.cmp(&right.name));
    recall.audits.truncate(limit);
    Ok((transaction, recall))
}
