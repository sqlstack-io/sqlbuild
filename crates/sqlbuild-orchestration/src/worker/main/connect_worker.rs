//! Bound worker requests have deadlines and must never follow credential-bearing redirects.

use reqwest::header::HeaderValue;
use uuid::Uuid;

use crate::worker::_helpers::response::http_client;
use crate::worker::errors::WorkerError;
use crate::worker::models::WorkerClient;

fn worker_url(server_url: &str, run_id: Uuid) -> Result<reqwest::Url, WorkerError> {
    let mut url = reqwest::Url::parse(server_url)
        .map_err(|_| WorkerError::Configuration("server URL must be an absolute HTTP URL"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(WorkerError::Configuration(
            "server URL must use HTTP or HTTPS without credentials, query or fragment",
        ));
    }
    let prefix = url.path().trim_end_matches('/');
    url.set_path(&format!("{prefix}/api/runs/{run_id}/"));
    Ok(url)
}

/// Connect to an API origin or path prefix using the launcher's run-scoped bearer.
pub fn connect_worker(
    server_url: &str,
    bearer: &str,
    (run_id, claim_id): (Uuid, Uuid),
) -> Result<WorkerClient, WorkerError> {
    let run_url = worker_url(server_url, run_id)?;
    let mut authorization = HeaderValue::from_str(&format!("Bearer {bearer}"))
        .map_err(|_| WorkerError::Configuration("run bearer is not a valid HTTP credential"))?;
    authorization.set_sensitive(true);
    let client = http_client(authorization.clone())?;
    Ok(WorkerClient {
        client,
        authorization,
        run_url,
        run_id,
        claim_id,
    })
}
