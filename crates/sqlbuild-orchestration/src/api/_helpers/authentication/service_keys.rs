//! A service account's key authenticates person routes from any peer, whatever the sign-in mode.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use axum::Json;
use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::api::_helpers::authentication::password_transport::{
    deny, private_cache_control, single_header,
};
use crate::api::errors::ApiError;
use crate::api::models::{KeyRefusals, LoginMode, LoginModeAnswer};
use crate::credentials::constants::{KEY_REFUSAL_INTERVAL_SECONDS, SERVICE_KEY_PREFIX};
use crate::credentials::main::service_keys::authenticate_service_key::authenticate_service_key;
use crate::credentials::main::service_keys::record_refused_key::record_refused_key;
use crate::credentials::main::service_keys::record_service_key_use::record_service_key_use;
use crate::credentials::models::{
    AuditContext, AuthenticatedPerson, DefaultRole, ServiceKeyCheck, ServiceKeyIdentity,
};
use crate::store::models::ControlStore;

/// Forget refusals beyond this many keys; at worst a refusal is audited again sooner.
const REFUSALS_KEPT: usize = 10_000;

/// The key in a single `Authorization: Bearer sqbk_…` header; a proxy's own bearer token is no key.
pub(crate) fn presented_key(headers: &HeaderMap) -> Option<&str> {
    single_header(headers, header::AUTHORIZATION.as_str())?
        .strip_prefix("Bearer ")
        .filter(|credential| credential.starts_with(SERVICE_KEY_PREFIX))
}

/// `routes`, also open to service keys; a request without one goes on to the mode's own sign-in.
pub(crate) fn keyed(store: &ControlStore, routes: Router<ControlStore>) -> Router<ControlStore> {
    let state = (store.clone(), KeyRefusals::default());
    routes.route_layer(middleware::from_fn_with_state(state, optional))
}

/// `routes` on a server without people's sign-in: only service keys get in.
pub(crate) fn key_only(store: &ControlStore, routes: Router<ControlStore>) -> Router<ControlStore> {
    let state = (store.clone(), KeyRefusals::default());
    routes.route_layer(middleware::from_fn_with_state(state, required))
}

async fn optional(
    State(state): State<(ControlStore, KeyRefusals)>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    match presented_key(request.headers()).map(str::to_owned) {
        Some(key) => keyed_request(&state, (request, next), &key).await,
        None => Ok(next.run(request).await),
    }
}

async fn required(
    State(state): State<(ControlStore, KeyRefusals)>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let mut response = match presented_key(request.headers()).map(str::to_owned) {
        Some(key) => keyed_request(&state, (request, next), &key).await?,
        None => deny(&context_of(&request))?,
    };
    private_cache_control(&mut response);
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

/// Runs the request as the key's service account, without the default role; a known key refused is audited.
async fn keyed_request(
    (store, refusals): &(ControlStore, KeyRefusals),
    (mut request, next): (Request, Next),
    key: &str,
) -> Result<Response, ApiError> {
    let mut context = context_of(&request);
    let now = Utc::now();
    match authenticate_service_key(store, key, now).await? {
        ServiceKeyCheck::Accepted(identity) => {
            context.actor = format!("principal:{}", identity.principal_id);
            context.granting_role = "service-key".into();
            let extensions = request.extensions_mut();
            extensions.insert(context);
            extensions.insert(AuthenticatedPerson(identity.principal_id, None));
            extensions.insert(DefaultRole::None);
            extensions.insert(identity);
            let response = next.run(request).await;
            note_use(store, (identity, response.status()), now).await;
            Ok(response)
        }
        ServiceKeyCheck::Refused {
            principal_id,
            key_id,
            reason,
        } => {
            if refusal_due(refusals, key_id) {
                record_refused_key(store, &context, (principal_id, key_id, reason)).await?;
            }
            deny(&context)
        }
        ServiceKeyCheck::Unknown => deny(&context),
    }
}

/// Moves the key's last use for a request that wasn't refused; a failed write never fails the call it describes.
async fn note_use(
    store: &ControlStore,
    (identity, status): (ServiceKeyIdentity, StatusCode),
    now: DateTime<Utc>,
) {
    if !matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        drop(record_service_key_use(store, identity, now).await);
    }
}

/// The sign-in guard's context, or one built from the socket peer where no guard ran.
fn context_of(request: &Request) -> AuditContext {
    request
        .extensions()
        .get::<AuditContext>()
        .cloned()
        .unwrap_or_else(|| AuditContext {
            actor: "anonymous".into(),
            granting_role: "authentication".into(),
            request_id: Uuid::now_v7(),
            ip_address: request
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ConnectInfo(peer)| peer.ip().to_canonical()),
        })
}

/// Whether `key_id`'s refusal is audited now: at most once an interval, so a retrying script can't flood the log.
fn refusal_due(refusals: &KeyRefusals, key_id: Uuid) -> bool {
    let Ok(mut seen) = refusals.0.lock() else {
        return true;
    };
    let now = Instant::now();
    let interval = Duration::from_secs(KEY_REFUSAL_INTERVAL_SECONDS);
    if seen
        .get(&key_id)
        .is_some_and(|last| now.duration_since(*last) < interval)
    {
        return false;
    }
    if seen.len() >= REFUSALS_KEPT {
        seen.clear();
    }
    seen.insert(key_id, now);
    true
}

#[utoipa::path(get, path = "/api/auth/login-mode", operation_id = "readLoginMode",
    description = "Answered only by a server without sign-in for people, so a browser or sqb login can say so instead of offering a password.",
    responses((status = 200, body = LoginModeAnswer), (status = 404, description = "People sign in on this server")))]
pub(crate) async fn keys_only_mode() -> Json<LoginModeAnswer> {
    Json(LoginModeAnswer {
        login_mode: LoginMode::Keys,
    })
}
