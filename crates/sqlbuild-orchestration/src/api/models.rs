//! Typed HTTP responses shared with the generated OpenAPI contract.

use serde::{Deserialize, Serialize};

use crate::api::_helpers::{run_reads, runs, snapshots, trigger_reads};

#[derive(Debug, utoipa::OpenApi)]
#[openapi(paths(
    snapshots::publish,
    snapshots::current,
    runs::execution,
    runs::report,
    runs::heartbeat,
    runs::output,
    run_reads::recent,
    run_reads::nodes,
    trigger_reads::current,
    trigger_reads::events,
    trigger_reads::ticks
))]
pub(crate) struct ControlApi;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadQuery {
    pub(crate) limit: Option<u32>,
}

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

#[derive(Clone, Debug)]
pub(crate) struct ControlClient {
    pub(crate) client: reqwest::Client,
    pub(crate) authorization: reqwest::header::HeaderValue,
    pub(crate) base_url: reqwest::Url,
}
