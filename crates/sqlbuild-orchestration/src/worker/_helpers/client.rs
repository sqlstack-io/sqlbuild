//! A worker's authenticated HTTP requests carry only its own run and claim.

use sqlbuild_recall::records::models::OutputChunk;
use uuid::Uuid;

use crate::api::errors::ClientError;
use crate::api::main::client_response::client_response;
use crate::api::models::{HeartbeatResponse, MutationResponse};
use crate::runs::models::{OutputPublication, RunChange, RunChangeRequest, RunExecution};
use crate::worker::errors::WorkerError;
use crate::worker::models::WorkerClient;

/// Decode a run execution, refusing one with any part this release can't read: a field it would drop at any depth, or a field or variant it can't decode.
pub(crate) fn read_execution(body: serde_json::Value) -> Result<RunExecution, WorkerError> {
    let mut ignored: Option<String> = None;
    let mut note = |path: serde_ignored::Path<'_>| {
        ignored.get_or_insert_with(|| path.to_string());
    };
    let decoded = serde_path_to_error::deserialize::<_, RunExecution>(
        serde_ignored::Deserializer::new(body, &mut note),
    );
    match (decoded, ignored) {
        (Err(error), _) => Err(WorkerError::UnreadableExecution(unreadable_part(
            &error.path().to_string(),
            &error.inner().to_string(),
        ))),
        (Ok(_), Some(field)) => Err(WorkerError::UnreadableExecution(format!(
            "unknown field `{field}`"
        ))),
        (Ok(execution), None) => Ok(execution),
    }
}

/// Serde's message without the list of what it expected, then where in the execution it failed, such as ``unknown variant `x` in `claim.request.command` ``.
fn unreadable_part(path: &str, message: &str) -> String {
    let head: &str = message.split(", expected").next().unwrap_or(message);
    match path {
        "" | "." => head.to_owned(),
        path => format!("{head} in `{path}`"),
    }
}

impl WorkerClient {
    /// Fetch the launcher's immutable request, not the location's latest deployment; a field this release can't read refuses it rather than run part of what was asked.
    pub async fn execution(&self) -> Result<RunExecution, WorkerError> {
        let body: serde_json::Value = client_response(
            self.connection
                .client
                .get(format!("{}execution", self.run_url)),
        )
        .await?;
        let execution: RunExecution = read_execution(body)?;
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
        .await
        .map_err(|error| match error {
            ClientError::Rejected(reqwest::StatusCode::UNPROCESSABLE_ENTITY) => {
                WorkerError::OlderServer
            }
            other => other.into(),
        })?;
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

    /// Send one batch of run output; a retried batch is stored once.
    pub async fn publish_output_batch(
        &self,
        batch: &crate::run_output::models::OutputBatch,
    ) -> Result<crate::run_output::models::OutputBatchReceipt, ClientError> {
        client_response(
            self.connection
                .client
                .post(format!("{}output/batches", self.run_url))
                .json(batch),
        )
        .await
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
