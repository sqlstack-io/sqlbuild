//! Typed HTTP responses shared with the generated OpenAPI contract.

use serde::{Deserialize, Serialize};

use crate::api::_helpers::{runs, snapshots};

#[derive(Debug, utoipa::OpenApi)]
#[openapi(paths(
    snapshots::publish,
    snapshots::current,
    runs::execution,
    runs::report,
    runs::heartbeat
))]
pub(crate) struct ControlApi;

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MutationResponse {
    pub applied: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct HeartbeatResponse {
    pub cancel_requested: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ApiErrorBody {
    pub error: String,
}
