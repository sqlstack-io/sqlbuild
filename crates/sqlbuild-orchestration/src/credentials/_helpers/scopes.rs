//! Service credentials stop being valid with their live location or run claim.

use crate::credentials::models::CredentialScope;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Bound values in location/run/claim column order for the credential insert.
pub(crate) fn scope_parameters(scope: CredentialScope) -> Vec<Parameter> {
    match scope {
        CredentialScope::Location { location_id } => [Some(location_id), None, None],
        CredentialScope::Run { run_id, claim_id } => [None, Some(run_id), Some(claim_id)],
    }
    .into_iter()
    .map(Parameter::OptionalId)
    .collect()
}

pub(crate) async fn scope_is_active(
    transaction: ControlTransaction,
    scope: CredentialScope,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (sql, parameters) = match scope {
        CredentialScope::Location { location_id } => (
            "SELECT location_id FROM sqb_locations WHERE location_id=$1 AND archived_at IS NULL",
            vec![Parameter::Id(location_id)],
        ),
        CredentialScope::Run { run_id, claim_id } => (
            "SELECT run_id FROM sqb_runs WHERE run_id=$1 AND worker_instance=$2",
            vec![Parameter::Id(run_id), Parameter::Id(claim_id)],
        ),
    };
    let (transaction, rows) = query(transaction, sql, parameters).await?;
    Ok((transaction, !rows.is_empty()))
}
