//! A person who may view a target reads past runs' results for the run selectors of a command they run elsewhere.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::_helpers::lifecycle::publications::viewed_publication;
use crate::runs::_helpers::requests::past_runs::read_past_runs;
use crate::runs::models::PastRunsRead;
use crate::runs::types::CommandPlanner;
use crate::runs::types::PastRuns;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};

/// The runs `ids` name in the location and target, each by the ID as given, for a person who may view the target.
pub async fn read_person_past_runs(
    store: &ControlStore,
    (planner, credentials): (&dyn CommandPlanner, (&AuditContext, Uuid)),
    (location_id, target, ids): (Uuid, &str, &BTreeSet<String>),
    default_role: DefaultRole,
) -> Result<PastRunsRead, ControlError> {
    viewed_publication(
        store,
        (planner, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, past) = read_past_runs(transaction, (location_id, target), ids).await?;
    commit(transaction).await?;
    Ok(match past {
        Ok(runs) => PastRunsRead { runs, error: None },
        Err(error) => PastRunsRead {
            runs: PastRuns::new(),
            error: Some(error),
        },
    })
}
