//! Typed HTTP responses shared with the generated OpenAPI contract.

use serde::{Deserialize, Serialize};

use crate::api::_helpers::{
    location_health, location_work, run_reads, runs, snapshots, trigger_reads,
};

#[derive(Debug, utoipa::OpenApi)]
#[openapi(paths(
    location_health::report,
    location_health::recent,
    location_work::poll,
    location_work::complete,
    location_work::result,
    snapshots::publish,
    snapshots::current,
    runs::execution,
    runs::report,
    runs::heartbeat,
    runs::output,
    runs::recall,
    run_reads::recent,
    run_reads::nodes,
    run_reads::invocations,
    run_reads::recall,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunRecallQuery {
    pub(crate) invocation: String,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RunRecallPath {
    pub(crate) location_id: uuid::Uuid,
    pub(crate) target: String,
    pub(crate) run_id: uuid::Uuid,
    pub(crate) view: String,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ReadResource {
    Run,
    Trigger,
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
