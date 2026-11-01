//! Proxy subjects resolve only through explicit server-owned bindings and the real socket peer.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json, Router, middleware};
use serde_json::json;

use crate::api::_helpers::authentication::password_transport::{
    deny, guard, parse_origin, single_header,
};
use crate::api::constants::PROXY_USER_HEADER;
use crate::api::errors::ApiError;
use crate::api::models::{AuthenticationDenial, ProxyIdentityResponse, TrustedProxyAuthentication};
use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) fn router(
    store: &ControlStore,
    settings: Option<TrustedProxyAuthentication>,
) -> Result<Router<ControlStore>, ControlError> {
    let Some(mut settings) = settings else {
        return Ok(Router::new());
    };
    let origin = parse_origin(&settings.origin, "trusted-proxy")?;
    if settings.trusted_peers.is_empty() {
        return Err(ControlError::Invalid(
            "trusted-proxy authentication requires explicit trusted peer IP addresses".into(),
        ));
    }
    if settings.identities.keys().any(|subject| {
        subject.trim().is_empty()
            || subject.trim() != subject
            || match HeaderValue::from_str(subject) {
                Ok(value) => value.to_str().is_err(),
                Err(_) => true,
            }
    }) {
        return Err(ControlError::Invalid(
            "trusted-proxy subjects must be nonempty, unpadded ASCII header values".into(),
        ));
    }
    settings.trusted_peers = settings
        .trusted_peers
        .into_iter()
        .map(|peer| peer.to_canonical())
        .collect();
    Ok(Router::new()
        .route("/api/auth/proxy", get(identity))
        .layer(middleware::from_fn_with_state(store.clone(), guard))
        .layer(Extension(origin))
        .layer(Extension(Arc::new(settings))))
}

#[utoipa::path(get, path = "/api/auth/proxy", operation_id = "proxyIdentity",
    description = "Requires a configured socket peer and an upstream-replaced X-SQB-User subject. Does not issue sessions or grant permissions.",
    security(("proxy" = [])),
    responses((status = 200, body = ProxyIdentityResponse), (status = 401, body = AuthenticationDenial)))]
pub(crate) async fn identity(
    State(store): State<ControlStore>,
    Extension(settings): Extension<Arc<TrustedProxyAuthentication>>,
    Extension(mut context): Extension<AuditContext>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if !context
        .ip_address
        .is_some_and(|peer| settings.trusted_peers.contains(&peer))
    {
        return deny(&store, &context, "untrusted_proxy_peer").await;
    }
    let principal = single_header(&headers, PROXY_USER_HEADER)
        .and_then(|subject| settings.identities.get(subject))
        .copied();
    let Some(principal_id) = principal else {
        return deny(&store, &context, "unbound_proxy_subject").await;
    };
    let transaction = begin(&store, TransactionMode::Write).await?;
    let (transaction, rows) = query(transaction,
        "SELECT principal_id FROM sqb_principals WHERE principal_id=$1 AND kind='person' AND disabled_at IS NULL",
        vec![Parameter::Id(principal_id)]).await?;
    if rows.is_empty() {
        commit(transaction).await?;
        return deny(&store, &context, "inactive_proxy_person").await;
    }
    context.actor = format!("principal:{principal_id}");
    context.granting_role = "trusted-proxy".into();
    let identity = ProxyIdentityResponse { principal_id };
    commit(
        append_audit(
            transaction,
            &context,
            (context.request_id, "proxy.authenticated"),
            (
                json!({"principal_id": principal_id}),
                json!(null),
                json!(&identity),
            ),
        )
        .await?,
    )
    .await?;
    Ok(Json(identity).into_response())
}
