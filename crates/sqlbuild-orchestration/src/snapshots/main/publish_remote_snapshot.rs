//! A code location sends retained compiled data over HTTP and never opens the control store.

use crate::api::errors::ClientError;
use crate::api::main::client_response::client_response;
use crate::api::main::connect_client::connect_client;
use crate::snapshots::models::{SnapshotInput, SnapshotRecord};

pub async fn publish_remote_snapshot(
    server_url: &str,
    bearer: &str,
    input: &SnapshotInput,
) -> Result<SnapshotRecord, ClientError> {
    let connection = connect_client(server_url, bearer)?;
    let url = connection.endpoint(&[
        "api",
        "locations",
        &input.location_id.to_string(),
        "targets",
        &input.target,
        "snapshots",
    ])?;
    client_response(connection.client.post(url).json(input)).await
}
