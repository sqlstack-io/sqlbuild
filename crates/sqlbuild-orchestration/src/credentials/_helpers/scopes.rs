//! Service credentials stop being valid with their live principal, location, or run claim.

use crate::credentials::models::CredentialScope;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Bound values in principal/location/run/claim column order for the credential insert.
pub(crate) fn scope_parameters(scope: CredentialScope) -> Vec<Parameter> {
    match scope {
        CredentialScope::Principal { principal_id } => [Some(principal_id), None, None, None],
        CredentialScope::Location { location_id } => [None, Some(location_id), None, None],
        CredentialScope::Run { run_id, claim_id } => [None, None, Some(run_id), Some(claim_id)],
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
        CredentialScope::Principal { principal_id } => (
            "SELECT principal_id FROM sqb_principals WHERE principal_id=$1 AND disabled_at IS NULL",
            vec![Parameter::Id(principal_id)],
        ),
        CredentialScope::Location { location_id } => (
            "SELECT location_id FROM sqb_locations WHERE location_id=$1 AND archived_at IS NULL",
            vec![Parameter::Id(location_id)],
        ),
        CredentialScope::Run { run_id, claim_id } => (
            "SELECT run_id FROM sqb_run_liveness WHERE run_id=$1 AND worker_instance=$2",
            vec![Parameter::Id(run_id), Parameter::Id(claim_id)],
        ),
    };
    let (transaction, rows) = query(transaction, sql, parameters).await?;
    Ok((transaction, !rows.is_empty()))
}
