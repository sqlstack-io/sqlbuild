//! The discovered sets of a location's current snapshot on the server, read with its credential.

use reqwest::StatusCode;
use serde_json::Value;
use uuid::Uuid;

use crate::api::errors::ClientError;
use crate::api::main::client_response::client_response;
use crate::api::main::connect_client::connect_client;
use crate::discovered_sets::constants::DISCOVERED_DOCUMENT;
use crate::discovered_sets::types::DiscoveredSets;

/// The sets the current snapshot of `target` saved; empty when there is none or it saved none.
pub async fn remote_discovered_sets(
    (server_url, bearer): (&str, &str),
    (location_id, target): (Uuid, &str),
) -> Result<DiscoveredSets, ClientError> {
    let connection = connect_client(server_url, bearer)?;
    let url = connection.endpoint(&[
        "api",
        "locations",
        &location_id.to_string(),
        "targets",
        target,
        "snapshots",
    ])?;
    let current: Value = match client_response(connection.client.get(url)).await {
        Ok(current) => current,
        Err(ClientError::Rejected(status)) if status == StatusCode::NOT_FOUND => {
            return Ok(DiscoveredSets::new());
        }
        Err(error) => return Err(error),
    };
    current["documents"]
        .get(DISCOVERED_DOCUMENT)
        .cloned()
        .map_or_else(
            || Ok(DiscoveredSets::new()),
            |document| {
                serde_json::from_value(document).map_err(|_| {
                    ClientError::Configuration(
                        "the current snapshot's discovered sets are unreadable",
                    )
                })
            },
        )
}
