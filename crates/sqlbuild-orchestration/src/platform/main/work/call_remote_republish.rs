//! A keyed caller asks for a republish or reads one back over the bearer transport, HTTP or HTTPS.

use uuid::Uuid;

use crate::api::errors::ClientError;
use crate::api::main::client_response::client_response;
use crate::api::main::connect_client::connect_client;
use crate::platform::models::{RepublishCall, RepublishStatus};

/// The republish as `key`'s principal sees it after `call`; `None` when there is no such request.
pub async fn call_remote_republish(
    (server_url, key): (&str, &str),
    (location_id, target): (Uuid, &str),
    call: RepublishCall<'_>,
) -> Result<Option<RepublishStatus>, ClientError> {
    let connection = connect_client(server_url, key)?;
    let location: String = location_id.to_string();
    let base = [
        "api",
        "control",
        "locations",
        &location,
        "targets",
        target,
        "republish",
    ];
    let request = match call {
        RepublishCall::Request(request) => connection
            .client
            .post(connection.endpoint(&base)?)
            .json(request),
        RepublishCall::Read { request_id, wait } => {
            let request_id: String = request_id.to_string();
            let url = connection.endpoint(&[&base[..], &[request_id.as_str()]].concat())?;
            connection.client.get(url).query(&[("wait", wait)])
        }
    };
    client_response(request).await
}
