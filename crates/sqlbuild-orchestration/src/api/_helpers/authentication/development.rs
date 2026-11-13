//! Development person requests: `local_request` already confined them to a loopback host and origin.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

use crate::credentials::models::{AuditContext, AuthenticatedPerson};

pub(crate) async fn authenticate(
    State(principal_id): State<Uuid>,
    mut request: Request,
    next: Next,
) -> Response {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip().to_canonical());
    request.extensions_mut().insert(AuditContext {
        actor: format!("principal:{principal_id}"),
        granting_role: "development".into(),
        request_id: Uuid::now_v7(),
        ip_address: peer,
    });
    request
        .extensions_mut()
        .insert(AuthenticatedPerson(principal_id, None));
    next.run(request).await
}
