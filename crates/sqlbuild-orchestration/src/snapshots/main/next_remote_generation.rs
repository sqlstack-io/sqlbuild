//! Reconcile publication generations from the authenticated server, including lost acknowledgements.

use uuid::Uuid;

use crate::api::errors::ClientError;
use crate::api::main::client_response::client_response;
use crate::api::main::connect_client::connect_client;
use crate::snapshots::models::SnapshotRecord;

pub async fn next_remote_generation(
    server_url: &str,
    bearer: &str,
    location_id: Uuid,
    target: &str,
) -> Result<i64, ClientError> {
    let connection = connect_client(server_url, bearer)?;
    let url = connection.endpoint(&[
        "api",
        "locations",
        &location_id.to_string(),
        "targets",
        target,
        "snapshots",
        "metadata",
    ])?;
    match client_response::<SnapshotRecord>(connection.client.get(url)).await {
        Ok(current) => current
            .generation
            .checked_add(1)
            .ok_or(ClientError::Configuration(
                "snapshot generation is exhausted",
            )),
        Err(ClientError::Rejected(status)) if status == reqwest::StatusCode::NOT_FOUND => Ok(1),
        Err(error) => Err(error),
    }
}
