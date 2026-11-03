//! SQL-only inspection uses live person grants, not the machine snapshot endpoint.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::_helpers::authentication::password_transport::session_credentials;
use crate::api::errors::ApiError;
use crate::credentials::models::AuditContext;
use crate::snapshots::main::read_person_node_sql::read_person_node_sql;
use crate::snapshots::models::NodeSql;
use crate::store::models::ControlStore;

#[utoipa::path(get, path = "/api/control/locations/{location_id}/targets/{target}/nodes/{node_key}/sql", operation_id = "readPersonNodeSql",
    params(("location_id" = Uuid, Path), ("target" = String, Path), ("node_key" = String, Path)),
    responses((status = 200, body = Option<NodeSql>), (status = 401, description = "Session rejected"),
        (status = 403, body = crate::api::models::ApiErrorBody)), security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn read(
    (State(store), Extension(context)): (State<ControlStore>, Extension<AuditContext>),
    Path((location_id, target, node_key)): Path<(Uuid, String, String)>,
    headers: HeaderMap,
) -> Result<Json<Option<NodeSql>>, ApiError> {
    Ok(Json(
        read_person_node_sql(
            &store,
            (&context, session_credentials(&headers)),
            (location_id, &target, &node_key),
        )
        .await?,
    ))
}
