//! The authenticated client verifies every leased request's location, target and instance.

use uuid::Uuid;

use crate::api::errors::ClientError;
use crate::api::main::client_response::client_response;
use crate::api::models::MutationResponse;
use crate::platform::models::{
    LocationWorkClaim, LocationWorkClient, LocationWorkPoll, LocationWorkResult,
};

impl LocationWorkClient {
    pub async fn poll(
        &self,
        poll: &LocationWorkPoll,
    ) -> Result<Option<LocationWorkClaim>, ClientError> {
        let claim: Option<LocationWorkClaim> = client_response(
            self.connection
                .client
                .post(format!("{}poll", self.work_url))
                .json(poll),
        )
        .await?;
        if claim.as_ref().is_some_and(|claim| {
            claim.request.location_id != self.location_id
                || claim.request.target != poll.target
                || claim.session_id != poll.session_id
        }) {
            return Err(ClientError::Configuration(
                "work response does not match this location, target and instance",
            ));
        }
        Ok(claim)
    }

    /// Retain the same request, claim and result when retrying an uncertain acknowledgement.
    pub async fn complete(
        &self,
        request_id: Uuid,
        result: &LocationWorkResult,
    ) -> Result<bool, ClientError> {
        let response: MutationResponse = client_response(
            self.connection
                .client
                .post(format!("{}{request_id}", self.work_url))
                .json(result),
        )
        .await?;
        Ok(response.applied)
    }
}
