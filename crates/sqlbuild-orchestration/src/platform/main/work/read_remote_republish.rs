//! A keyed caller reads a republish back over the bearer transport.

use uuid::Uuid;

use crate::api::errors::ClientError;
use crate::api::main::client_response::client_response;
use crate::api::main::connect_client::connect_client;
use crate::platform::models::RepublishStatus;

/// The republish, held open up to `wait` seconds while it is pending; `None` when there is none.
pub async fn read_remote_republish(
    (server_url, key): (&str, &str),
    (location_id, target, request_id): (Uuid, &str, Uuid),
    wait: u64,
) -> Result<Option<RepublishStatus>, ClientError> {
    let connection = connect_client(server_url, key)?;
    let url = connection.endpoint(&[
        "api",
        "control",
        "locations",
        &location_id.to_string(),
        "targets",
        target,
        "republish",
        &request_id.to_string(),
    ])?;
    client_response(connection.client.get(url).query(&[("wait", wait)])).await
}
