//! Only a digest of the CLI-held exchange secret is sent when starting a login.

use chrono::{Duration, SubsecRound};
use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::credentials::_helpers::secrets::new_bearer;
use crate::credentials::constants::{OIDC_FLOW_SECONDS, TOKEN_SECRET_HEX_LENGTH};
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, StartedLogin};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) async fn start_login(
    store: &ControlStore,
    context: &AuditContext,
    client_hash: &str,
) -> Result<StartedLogin, ControlError> {
    if client_hash.len() != TOKEN_SECRET_HEX_LENGTH
        || !client_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ControlError::Invalid("invalid CLI exchange digest".into()));
    }
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let id = Uuid::now_v7();
    let browser_code = new_bearer(id)?;
    let expires_at = (now + Duration::seconds(OIDC_FLOW_SECONDS)).trunc_subsecs(6);
    let (transaction, _) = query(transaction,
        "DELETE FROM sqb_cli_login_flows WHERE flow_id IN \
         (SELECT flow_id FROM sqb_cli_login_flows WHERE expires_at<=$1 ORDER BY expires_at LIMIT 1000)",
        vec![Parameter::Time(now)]).await?;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_cli_login_flows (flow_id,client_hash,browser_hash,expires_at) VALUES ($1,$2,$3,$4)",
        vec![Parameter::Id(id), Parameter::Text(client_hash.into()),
            Parameter::Text(sha256_hex(browser_code.as_bytes())), Parameter::Time(expires_at)]).await?;
    commit(
        append_audit(
            transaction,
            context,
            (id, "cli.login.started"),
            (
                json!({"flow_id":id}),
                json!(null),
                json!({"expires_at":expires_at}),
            ),
        )
        .await?,
    )
    .await?;
    Ok(StartedLogin {
        browser_code,
        expires_at,
    })
}
