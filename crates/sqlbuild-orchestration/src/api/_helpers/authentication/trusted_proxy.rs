//! Each request's proxy subject resolves through explicit bindings and the real socket peer.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use axum::{Extension, Router, middleware};

use crate::api::_helpers::authentication::password_sessions;
use crate::api::_helpers::authentication::password_transport::{self, deny, guard, single_header};
use crate::api::constants::PROXY_USER_HEADER;
use crate::api::errors::ApiError;
use crate::api::main::parse_origin::parse_origin;
use crate::api::models::{BrowserLoginMethod, BrowserOrigin, TrustedProxyAuthentication};
use crate::credentials::models::{AuditContext, AuthenticatedPerson};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) fn router(
    store: &ControlStore,
    mut settings: TrustedProxyAuthentication,
) -> Result<Router<ControlStore>, ControlError> {
    let origin = BrowserOrigin(parse_origin(&settings.origin, "trusted-proxy")?);
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
        .merge(
            password_sessions::person_routes()
                .route_layer(middleware::from_fn_with_state(store.clone(), authenticate)),
        )
        .merge(password_sessions::session_routes(store))
        .merge(super::cli_login::router(BrowserLoginMethod::Proxy))
        .layer(middleware::from_fn(guard))
        .layer(Extension(origin))
        .layer(Extension(Arc::new(settings))))
}

/// A CLI session credential authenticates as a session; otherwise the bound proxy subject does.
async fn authenticate(
    State(store): State<ControlStore>,
    Extension(settings): Extension<Arc<TrustedProxyAuthentication>>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if password_transport::session_header(request.headers()) {
        return password_transport::authenticate(State(store), request, next).await;
    }
    let Some(mut context) = request.extensions().get::<AuditContext>().cloned() else {
        return Err(
            ControlError::Invalid("trusted authentication context is unavailable".into()).into(),
        );
    };
    if !context
        .ip_address
        .is_some_and(|peer| settings.trusted_peers.contains(&peer))
    {
        return deny(&context);
    }
    let Some(principal_id) = single_header(request.headers(), PROXY_USER_HEADER)
        .and_then(|subject| settings.identities.get(subject))
        .copied()
    else {
        return deny(&context);
    };
    let (transaction, people) = query(
        begin(&store, TransactionMode::Read).await?,
        "SELECT principal_id FROM sqb_principals WHERE principal_id=$1 AND disabled_at IS NULL",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    commit(transaction).await?;
    if people.is_empty() {
        return deny(&context);
    }
    context.actor = format!("principal:{principal_id}");
    context.granting_role = "trusted-proxy".into();
    request.extensions_mut().insert(context);
    request
        .extensions_mut()
        .insert(AuthenticatedPerson(principal_id, None));
    Ok(next.run(request).await)
}
