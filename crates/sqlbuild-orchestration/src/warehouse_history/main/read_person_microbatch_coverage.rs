//! Live view permission over the whole target and one model's recorded microbatch coverage.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_history::_helpers::coverage::microbatch_coverage;
use crate::warehouse_history::models::{CoverageQuery, MicrobatchCoverage};

pub async fn read_person_microbatch_coverage(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    ((location_id, target), (model, options)): ((Uuid, &str), (&str, &CoverageQuery)),
    default_role: DefaultRole,
) -> Result<Option<MicrobatchCoverage>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        credentials,
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, coverage) = microbatch_coverage(
        transaction,
        (location_id, target),
        (model, options.limit.unwrap_or(200)),
    )
    .await?;
    commit(transaction).await?;
    Ok(coverage)
}
