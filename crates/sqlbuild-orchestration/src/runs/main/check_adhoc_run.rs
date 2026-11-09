//! A dry run resolves and decides a command exactly as a launch would, but queues and audits nothing.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::runs::_helpers::requests::adhoc::{
    frozen_adhoc_request, may_view_target, permission_decisions,
};
use crate::runs::models::{AdhocCheck, AdhocCheckRequest};
use crate::runs::types::CommandPlanner;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// Without view on every current node the dry run reveals nothing, and it never records a denial.
pub async fn check_adhoc_run(
    store: &ControlStore,
    (context, principal_id): (&AuditContext, Uuid),
    planner: &dyn CommandPlanner,
    request: &AdhocCheckRequest,
) -> Result<AdhocCheck, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, viewing) = may_view_target(transaction, principal_id, request).await?;
    let (transaction, frozen) = if viewing {
        frozen_adhoc_request(transaction, (context, planner), (request, Uuid::nil())).await?
    } else {
        (
            transaction,
            Err("view on every node of this target is required".to_owned()),
        )
    };
    let (verb, frozen) = match frozen {
        Ok(frozen) => frozen,
        Err(refusal) => {
            commit(transaction).await?;
            return Ok(AdhocCheck {
                snapshot_id: None,
                verb: None,
                selection: Vec::new(),
                write_set: Vec::new(),
                settings: Default::default(),
                permissions: Vec::new(),
                error: Some(refusal),
                launchable: false,
            });
        }
    };
    let (transaction, permissions) =
        permission_decisions(transaction, principal_id, &frozen).await?;
    commit(transaction).await?;
    Ok(AdhocCheck {
        snapshot_id: Some(frozen.snapshot_id),
        launchable: permissions.iter().all(|permission| permission.allowed),
        verb: Some(verb),
        selection: frozen.selection.into_iter().collect(),
        write_set: frozen.write_set.into_iter().collect(),
        settings: frozen.settings,
        permissions,
        error: None,
    })
}
