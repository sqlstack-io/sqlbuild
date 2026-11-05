//! An authenticated browser session approves or rejects one pending CLI login.

use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::credentials::_helpers::secrets::bearer_id;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, SessionIdentity};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) async fn approve_login(
    store: &ControlStore,
    context: &AuditContext,
    identity: &SessionIdentity,
    (browser_code, approve): (&str, bool),
) -> Result<bool, ControlError> {
    let Some(id) = bearer_id(browser_code) else {
        return Ok(false);
    };
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "SELECT flow_id FROM sqb_cli_login_flows WHERE flow_id=$1 AND browser_hash=$2 AND expires_at>$3 AND session_id IS NULL",
        vec![Parameter::Id(id), Parameter::Text(sha256_hex(browser_code.as_bytes())), Parameter::Time(now)]).await?;
    if rows.is_empty() {
        return Ok(false);
    }
    let (transaction, _) = if approve {
        query(
            transaction,
            "UPDATE sqb_cli_login_flows SET session_id=$2 WHERE flow_id=$1",
            vec![Parameter::Id(id), Parameter::Id(identity.session_id)],
        )
        .await?
    } else {
        query(
            transaction,
            "DELETE FROM sqb_cli_login_flows WHERE flow_id=$1",
            vec![Parameter::Id(id)],
        )
        .await?
    };
    commit(
        append_audit(
            transaction,
            context,
            (Uuid::new_v5(&id, b"decision"), "cli.login.decided"),
            (
                json!({"flow_id":id,"principal_id":identity.principal_id}),
                json!(null),
                json!({"approved":approve}),
            ),
        )
        .await?,
    )
    .await?;
    Ok(true)
}
