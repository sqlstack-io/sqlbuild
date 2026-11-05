//! Node outcomes are scoped to one frozen run, independently of current code snapshots.

use uuid::Uuid;

use crate::runs::models::RunNodeRecord;
use crate::store::errors::ControlError;
use crate::store::main::read_records::read_records;
use crate::store::models::{ControlStore, Parameter};

/// The API authorizes visibility of the run before reading its nodes.
pub async fn list_run_nodes(
    store: &ControlStore,
    run_id: Uuid,
) -> Result<Vec<RunNodeRecord>, ControlError> {
    read_records(store,
        "SELECT node_key, status, started_at, ended_at, rows_affected, error_code, skipped_because_of \
         FROM sqb_run_nodes WHERE run_id=$1 AND kind='node' ORDER BY node_key",
        vec![Parameter::Id(run_id)]).await
}
