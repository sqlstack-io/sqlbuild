//! A worker's authenticated HTTP requests carry only its own run and claim.

use sqlbuild_recall::records::models::OutputChunk;
use uuid::Uuid;

use crate::api::main::client_response::client_response;
use crate::api::models::{HeartbeatResponse, MutationResponse};
use crate::runs::models::{OutputPublication, RunChange, RunChangeRequest, RunExecution};
use crate::worker::errors::WorkerError;
use crate::worker::models::WorkerClient;

impl WorkerClient {
    /// Fetch the launcher's immutable request, not the location's latest deployment.
    pub async fn execution(&self) -> Result<RunExecution, WorkerError> {
        let execution: RunExecution = client_response(
            self.connection
                .client
                .get(format!("{}execution", self.run_url)),
        )
        .await?;
        if execution.claim.request.run_id != self.run_id
            || execution.claim.claim_id != self.claim_id
        {
            return Err(WorkerError::Configuration(
                "execution response does not match the worker's run and claim",
            ));
        }
        Ok(execution)
    }

    /// Call with the same event ID and payload when retrying an uncertain response.
    pub async fn report(&self, event_id: Uuid, change: RunChange) -> Result<bool, WorkerError> {
        let request = RunChangeRequest {
            event_id,
            run_id: self.run_id,
            claim_id: Some(self.claim_id),
            change,
        };
        let result: MutationResponse = client_response(
            self.connection
                .client
                .post(format!("{}events", self.run_url))
                .json(&request),
        )
        .await?;
        Ok(result.applied)
    }

    /// Retry the same chunks after an uncertain response; differing retries never overwrite bytes.
    pub async fn publish_output(
        &self,
        chunks: &[OutputChunk],
    ) -> Result<OutputPublication, WorkerError> {
        Ok(client_response(
            self.connection
                .client
                .post(format!("{}output", self.run_url))
                .json(chunks),
        )
        .await?)
    }

    /// Run independently from warehouse execution; cancellation is a cooperative request.
    pub async fn heartbeat(&self) -> Result<bool, WorkerError> {
        let result: HeartbeatResponse = client_response(
            self.connection
                .client
                .post(format!("{}heartbeat", self.run_url)),
        )
        .await?;
        Ok(result.cancel_requested)
    }
}
