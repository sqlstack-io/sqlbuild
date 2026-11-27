//! Finished runs past `run_output_days` lose their output; the run and its node logs stay.

use crate::run_output::_helpers::store::prune::{
    expired_runs, prune_run, report_retention_failure,
};
use crate::run_output::models::{ExpiredRun, OutputStore};
use crate::store::models::ControlStore;

/// How many runs lost their output; a failure is reported on stderr and retried next pass, never stopping other retention.
pub async fn prune_run_output(store: &ControlStore, output: &OutputStore, days: u16) -> usize {
    let expired: Vec<ExpiredRun> = match expired_runs(store, days).await {
        Ok(expired) => expired,
        Err(error) => {
            report_retention_failure(None, &error);
            return 0;
        }
    };
    let mut pruned: usize = 0;
    for ExpiredRun { run_id } in &expired {
        match prune_run(store, output, *run_id).await {
            Ok(()) => pruned += 1,
            Err(error) => report_retention_failure(Some(*run_id), &error),
        }
    }
    pruned
}
