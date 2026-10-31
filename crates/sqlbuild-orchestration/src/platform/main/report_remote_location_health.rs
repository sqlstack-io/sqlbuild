//! The code location has only its scoped HTTP credential, never a store handle.

use uuid::Uuid;

use crate::api::errors::ClientError;
use crate::api::main::client_response::client_response;
use crate::api::main::connect_client::connect_client;
use crate::api::models::MutationResponse;
use crate::platform::models::LocationHealthReport;

pub async fn report_remote_location_health(
    (server_url, bearer): (&str, &str),
    location_id: Uuid,
    report: &LocationHealthReport,
) -> Result<bool, ClientError> {
    let connection = connect_client(server_url, bearer)?;
    let url = connection.endpoint(&["api", "locations", &location_id.to_string(), "health"])?;
    let response: MutationResponse =
        client_response(connection.client.post(url).json(report)).await?;
    Ok(response.applied)
}
