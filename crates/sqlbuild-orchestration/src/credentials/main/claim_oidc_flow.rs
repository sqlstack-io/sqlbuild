//! Atomically consume browser-bound flow state before any identity-provider exchange.

use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::credentials::_helpers::secrets::secret_matches;
use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::{AuditContext, StoredOidcFlow};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, decode_row::decode_row, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) async fn claim_oidc_flow(
    store: &ControlStore,
    context: &AuditContext,
    configuration_id: &str,
    (state, browser): (&str, &str),
) -> Result<Option<StoredOidcFlow>, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let (transaction, rows) = query(
        transaction,
        "SELECT flow_id, browser_hash, nonce, pkce_verifier FROM sqb_oidc_login_flows \
         WHERE state_hash=$1 AND configuration_id=$2 AND expires_at>$3",
        vec![
            Parameter::Text(sha256_hex(state.as_bytes())),
            Parameter::Text(configuration_id.into()),
            Parameter::Time(now),
        ],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        commit(transaction).await?;
        return Ok(None);
    };
    let flow: StoredOidcFlow = decode_row(row)?;
    if !secret_matches(browser, &flow.browser_hash) {
        commit(transaction).await?;
        return Ok(None);
    }
    let (transaction, _) = query(
        transaction,
        "DELETE FROM sqb_oidc_login_flows WHERE flow_id=$1",
        vec![Parameter::Id(flow.flow_id)],
    )
    .await?;
    commit(
        append_audit(
            transaction,
            context,
            (
                Uuid::new_v5(&flow.flow_id, b"consumed"),
                "oidc.flow.consumed",
            ),
            (
                json!({"flow_id": flow.flow_id}),
                json!({"pending": true}),
                json!({"pending": false}),
            ),
        )
        .await?,
    )
    .await?;
    Ok(Some(flow))
}
