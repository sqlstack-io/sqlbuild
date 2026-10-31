//! Bound worker requests have deadlines and must never follow credential-bearing redirects.

use uuid::Uuid;

use crate::api::main::connect_client::connect_client;
use crate::worker::errors::WorkerError;
use crate::worker::models::WorkerClient;

/// Connect to an API origin or path prefix using the launcher's run-scoped bearer.
pub fn connect_worker(
    server_url: &str,
    bearer: &str,
    (run_id, claim_id): (Uuid, Uuid),
) -> Result<WorkerClient, WorkerError> {
    let connection = connect_client(server_url, bearer)?;
    let run_url = connection.endpoint(&["api", "runs", &run_id.to_string(), ""])?;
    Ok(WorkerClient {
        connection,
        run_url,
        run_id,
        claim_id,
    })
}
