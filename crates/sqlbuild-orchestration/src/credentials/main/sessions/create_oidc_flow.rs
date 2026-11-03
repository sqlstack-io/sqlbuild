//! Create one-use browser-bound PKCE attempts with durable audit evidence.

use crate::credentials::constants::OIDC_FLOW_SECONDS;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, OidcFlowStart};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use chrono::Duration;
use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

pub(crate) async fn create_oidc_flow(
    store: &ControlStore,
    context: &AuditContext,
    configuration_id: &str,
    flow: &OidcFlowStart,
) -> Result<(), ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let (transaction, _) = query(transaction,
        "DELETE FROM sqb_oidc_login_flows WHERE flow_id IN \
         (SELECT flow_id FROM sqb_oidc_login_flows WHERE expires_at<=$1 ORDER BY expires_at LIMIT 1000)",
        vec![Parameter::Time(now)]).await?;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_oidc_login_flows (flow_id, configuration_id, state_hash, browser_hash, nonce, pkce_verifier, expires_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
        vec![Parameter::Id(context.request_id), Parameter::Text(configuration_id.into()),
            Parameter::Text(sha256_hex(flow.state.secret().as_bytes())),
            Parameter::Text(sha256_hex(flow.browser.secret().as_bytes())),
            Parameter::Text(flow.nonce.secret().clone()), Parameter::Text(flow.pkce_verifier.secret().clone()),
            Parameter::Time(now + Duration::seconds(OIDC_FLOW_SECONDS))]).await?;
    commit(
        append_audit(
            transaction,
            context,
            (context.request_id, "oidc.flow.created"),
            (
                json!({"flow_id": context.request_id}),
                json!(null),
                json!({"pending": true}),
            ),
        )
        .await?,
    )
    .await
}
