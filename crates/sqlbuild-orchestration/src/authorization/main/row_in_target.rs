//! A person reads one run or trigger, including a removed trigger, only with view on the whole target.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{
    ControlStore, ControlTransaction, Parameter, TargetRow, TransactionMode,
};

/// Authorizes first, then `None` (after committing) when the row is not in this location and target.
pub(crate) async fn row_in_target(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    ((location_id, target), row): ((Uuid, &str), TargetRow),
    default_role: DefaultRole,
) -> Result<Option<ControlTransaction>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        credentials,
        (location_id, target),
        default_role,
    )
    .await?;
    let (sql, id) = match row {
        TargetRow::Run(run_id) => (
            "SELECT run_id FROM sqb_runs WHERE run_id=$1 AND location_id=$2 AND target=$3",
            run_id,
        ),
        TargetRow::Trigger(trigger_id) => (
            "SELECT trigger_id FROM sqb_triggers WHERE trigger_id=$1 AND location_id=$2 AND target=$3",
            trigger_id,
        ),
    };
    let (transaction, rows) = query(
        transaction,
        sql,
        vec![
            Parameter::Id(id),
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
        ],
    )
    .await?;
    if rows.is_empty() {
        commit(transaction).await?;
        return Ok(None);
    }
    Ok(Some(transaction))
}
