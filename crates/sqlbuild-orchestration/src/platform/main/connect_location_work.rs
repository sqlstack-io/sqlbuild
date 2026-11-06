//! Reuse one credential-bearing pool for the location's outbound work channel.

use uuid::Uuid;

use crate::api::errors::ClientError;
use crate::api::main::connect_client::connect_client;
use crate::platform::models::LocationWorkClient;

pub fn connect_location_work(
    (server_url, bearer): (&str, &str),
    location_id: Uuid,
) -> Result<LocationWorkClient, ClientError> {
    let connection = connect_client(server_url, bearer)?;
    let work_url =
        connection.endpoint(&["api", "locations", &location_id.to_string(), "work", ""])?;
    Ok(LocationWorkClient {
        connection,
        work_url,
        location_id,
    })
}
