//! A code location sends retained compiled data over HTTP and never opens the control store.

use reqwest::StatusCode;
use reqwest::header::{CONTENT_ENCODING, CONTENT_TYPE};

use crate::api::constants::{BODY_BYTES, GZIP_ENCODING, JSON_MEDIA_TYPE};
use crate::api::errors::ClientError;
use crate::api::main::client_response::client_response;
use crate::api::main::connect_client::connect_client;
use crate::snapshots::models::{SnapshotInput, SnapshotRecord};
use crate::store::main::compress::compress;

/// Sends the snapshot gzip-encoded, and once more as plain JSON when a server refuses the encoding and the plain body fits the wire bound.
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
    let body = serde_json::to_vec(input)
        .map_err(|_| ClientError::Configuration("snapshot is not serializable as JSON"))?;
    let gzipped =
        compress(&body).map_err(|_| ClientError::Configuration("snapshot could not be gzipped"))?;
    let request = |body: Vec<u8>| {
        connection
            .client
            .post(url.clone())
            .header(CONTENT_TYPE, JSON_MEDIA_TYPE)
            .body(body)
    };
    match client_response(request(gzipped).header(CONTENT_ENCODING, GZIP_ENCODING)).await {
        Err(ClientError::Rejected(status))
            if (status == StatusCode::BAD_REQUEST
                || status == StatusCode::UNSUPPORTED_MEDIA_TYPE)
                && body.len() <= BODY_BYTES =>
        {
            client_response(request(body)).await
        }
        result => result,
    }
}
