//! Finished runs past `run_output_days` lose their output; the run and its node logs stay.

use chrono::TimeDelta;

use crate::run_output::_helpers::prune::prune_run;
use crate::run_output::constants::PRUNE_RUNS;
use crate::run_output::models::{ExpiredRun, OutputStore};
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::main::write::write;
use crate::store::models::{ControlStore, Parameter};

/// How many runs lost their output. A run whose output could not be removed is reported on stderr
/// and left for the next pass, so it never stops the others; a repeated or interrupted pass finds
/// only output still stored.
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
    let mut pruned: usize = 0;
    for ExpiredRun { run_id } in &expired {
        match prune_run(store, output, *run_id).await {
            Ok(()) => pruned += 1,
            Err(error) => eprintln!(
                "Run output retention: could not remove the output of run {run_id}; retrying next pass: {error}"
            ),
        }
    }
    Ok(pruned)
}
