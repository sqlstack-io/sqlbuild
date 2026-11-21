//! A service account's key authenticates person routes from any peer, whatever the sign-in mode.
//!
//! Keys are recognised by their prefix in `Authorization: Bearer`, so a proxy's own bearer token still
//! reaches the mode's sign-in. A keyed request never consults the trusted proxy's identity header.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use uuid::Uuid;

use crate::api::_helpers::authentication::password_transport::{
    deny, private_cache_control, single_header,
};
use crate::api::errors::ApiError;
use crate::credentials::_helpers::service_keys::is_service_key;
use crate::credentials::constants::KEY_REFUSAL_INTERVAL_SECONDS;
use crate::credentials::main::service_keys::authenticate_service_key::authenticate_service_key;
use crate::credentials::main::service_keys::record_refused_key::record_refused_key;
use crate::credentials::main::service_keys::record_service_key_use::record_service_key_use;
use crate::credentials::models::{AuditContext, AuthenticatedPerson, DefaultRole, ServiceKeyCheck};
use crate::store::models::ControlStore;

/// Keys whose refusal was audited lately, so a script retrying with a stopped key writes one entry a minute.
static REFUSALS: LazyLock<Mutex<HashMap<Uuid, Instant>>> = LazyLock::new(Mutex::default);
/// Forget refusals beyond this many keys; at worst a refusal is audited again sooner.
const REFUSALS_KEPT: usize = 10_000;

/// The key in a single `Authorization: Bearer sqbk_…` header.
pub(crate) fn presented_key(headers: &HeaderMap) -> Option<&str> {
    single_header(headers, header::AUTHORIZATION.as_str())?
        .strip_prefix("Bearer ")
        .filter(|credential| is_service_key(credential))
}

/// `routes`, also open to service keys; a request without one goes on to the mode's own sign-in.
pub(crate) fn keyed(store: &ControlStore, routes: Router<ControlStore>) -> Router<ControlStore> {
    routes.route_layer(middleware::from_fn_with_state(store.clone(), optional))
}

/// `routes` on a server without people's sign-in: only service keys get in.
pub(crate) fn key_only(store: &ControlStore, routes: Router<ControlStore>) -> Router<ControlStore> {
    routes.route_layer(middleware::from_fn_with_state(store.clone(), required))
}

async fn optional(
    State(store): State<ControlStore>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    match presented_key(request.headers()).map(str::to_owned) {
        Some(key) => keyed_request(&store, (request, next), &key).await,
        None => Ok(next.run(request).await),
    }
}

async fn required(
    State(store): State<ControlStore>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let mut response = match presented_key(request.headers()).map(str::to_owned) {
        Some(key) => keyed_request(&store, (request, next), &key).await?,
        None => deny(&context_of(&request))?,
    };
    private_cache_control(&mut response);
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

/// Runs the request as the key's service account, without the installation's default role.
///
/// The key's last use moves only when the request wasn't refused; a known key that is refused is audited.
async fn keyed_request(
    store: &ControlStore,
    (mut request, next): (Request, Next),
    key: &str,
) -> Result<Response, ApiError> {
    let mut context = context_of(&request);
    let now = chrono::Utc::now();
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
            if !matches!(
                response.status(),
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
            ) {
                // Last use is a hint for administrators; a failed write must not fail the call it describes.
                drop(record_service_key_use(store, identity, now).await);
            }
            Ok(response)
        }
        ServiceKeyCheck::Refused {
            principal_id,
            key_id,
            reason,
        } => {
            if refusal_due(key_id) {
                record_refused_key(store, &context, (principal_id, key_id, reason)).await?;
            }
            deny(&context)
        }
        ServiceKeyCheck::Unknown => deny(&context),
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

fn refusal_due(key_id: Uuid) -> bool {
    let Ok(mut refusals) = REFUSALS.lock() else {
        return true;
    };
    let now = Instant::now();
    let interval = Duration::from_secs(KEY_REFUSAL_INTERVAL_SECONDS);
    if refusals
        .get(&key_id)
        .is_some_and(|last| now.duration_since(*last) < interval)
    {
        return false;
    }
    if refusals.len() >= REFUSALS_KEPT {
        refusals.clear();
    }
    refusals.insert(key_id, now);
    true
}
