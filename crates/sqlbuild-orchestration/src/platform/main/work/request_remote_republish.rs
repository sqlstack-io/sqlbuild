//! A keyed caller asks a location to republish over the bearer transport, HTTP or HTTPS.

use uuid::Uuid;

use crate::api::errors::ClientError;
use crate::api::main::client_response::client_response;
use crate::api::main::connect_client::connect_client;
use crate::platform::models::{PersonWorkRequest, RepublishStatus};

/// Queue a republish as `key`'s principal; the status is where it stands straight after.
pub async fn request_remote_republish(
    (server_url, key): (&str, &str),
    (location_id, target): (Uuid, &str),
    request: &PersonWorkRequest,
) -> Result<RepublishStatus, ClientError> {
    let connection = connect_client(server_url, key)?;
    let url = connection.endpoint(&[
        "api",
        "control",
        "locations",
        &location_id.to_string(),
        "targets",
        target,
        "republish",
    ])?;
    client_response(connection.client.post(url).json(request)).await
}
