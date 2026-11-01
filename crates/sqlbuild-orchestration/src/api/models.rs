//! Typed HTTP responses shared with the generated OpenAPI contract.

use serde::{Deserialize, Serialize};

use crate::api::_helpers::{
    changes, location_health, location_work, run_reads, runs, snapshots, trigger_reads,
    warehouse_metadata,
};

#[derive(Debug, utoipa::OpenApi)]
#[openapi(paths(
    changes::subscribe,
    location_health::report,
    location_health::recent,
    location_work::poll,
    location_work::complete,
    location_work::result,
    snapshots::publish,
    snapshots::current,
    snapshots::metadata,
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
    trigger_reads::ticks,
    warehouse_metadata::report,
    warehouse_metadata::report_fingerprints
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

/// SSE invalidation topics; `all` means refetch the authorized REST resources.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChangeNotice {
    pub topic: String,
}

/// Explicit local development scope; never enabled by ordinary server startup.
#[derive(Clone, Debug)]
pub struct DevelopmentScope {
    pub location_id: uuid::Uuid,
    pub target: String,
    pub triggers: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ChangeFeed {
    pub(crate) sender: tokio::sync::broadcast::Sender<crate::store::models::ChangeMessage>,
    pub(crate) stopped: tokio::sync::watch::Receiver<bool>,
}

/// Sensitive headers redact the bearer from Debug output and all wire notifications.
#[derive(Debug)]
pub(crate) struct ChangeStream {
    pub(crate) store: crate::store::models::ControlStore,
    pub(crate) authorization: Option<axum::http::HeaderValue>,
    pub(crate) location_id: uuid::Uuid,
    pub(crate) target: String,
    pub(crate) target_hash: String,
    pub(crate) receiver: tokio::sync::broadcast::Receiver<crate::store::models::ChangeMessage>,
    pub(crate) stopped: tokio::sync::watch::Receiver<bool>,
    pub(crate) keepalive: tokio::time::Interval,
    pub(crate) initial: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ControlClient {
    pub(crate) client: reqwest::Client,
    pub(crate) authorization: reqwest::header::HeaderValue,
    pub(crate) base_url: reqwest::Url,
}
