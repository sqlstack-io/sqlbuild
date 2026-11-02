//! The authenticated client verifies every leased request's location, target and instance.

use uuid::Uuid;

use crate::api::errors::ClientError;
use crate::api::main::client_response::client_response;
use crate::api::models::MutationResponse;
use crate::platform::models::{
    LocationHealthReport, LocationWorkClaim, LocationWorkClient, LocationWorkPoll,
    LocationWorkResult,
};
use crate::warehouse_metadata::models::{
    AuditResultsBatch, FingerprintBatch, SourceFreshnessBatch, WarehouseSyncResponse,
    WarehouseWatermarkRead,
};

impl LocationWorkClient {
    pub async fn report_health(&self, report: &LocationHealthReport) -> Result<bool, ClientError> {
        let endpoint = self.connection.endpoint(&[
            "api",
            "locations",
            &self.location_id.to_string(),
            "health",
        ])?;
        let response: MutationResponse =
            client_response(self.connection.client.post(endpoint).json(report)).await?;
        Ok(response.applied)
    }

    pub async fn metadata_watermark(
        &self,
        target: &str,
        request: &WarehouseWatermarkRead,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, ClientError> {
        self.sync_metadata((target, "watermark"), request).await
    }

    pub async fn sync_fingerprints(
        &self,
        target: &str,
        batch: &FingerprintBatch,
    ) -> Result<WarehouseSyncResponse, ClientError> {
        self.sync_metadata((target, "fingerprints"), batch).await
    }

    pub async fn sync_audit_results(
        &self,
        target: &str,
        batch: &AuditResultsBatch,
    ) -> Result<WarehouseSyncResponse, ClientError> {
        self.sync_metadata((target, "audit-results"), batch).await
    }

    /// Metadata shares the location's scoped pool, not the scheduler's execution authority.
    pub async fn sync_source_freshness(
        &self,
        target: &str,
        batch: &SourceFreshnessBatch,
    ) -> Result<WarehouseSyncResponse, ClientError> {
        self.sync_metadata((target, "source-freshness"), batch)
            .await
    }

    async fn sync_metadata<T: serde::de::DeserializeOwned>(
        &self,
        (target, table): (&str, &str),
        batch: &impl serde::Serialize,
    ) -> Result<T, ClientError> {
        let endpoint = self.connection.endpoint(&[
            "api",
            "locations",
            &self.location_id.to_string(),
            "targets",
            target,
            "metadata",
            table,
        ])?;
        client_response(self.connection.client.post(endpoint).json(batch)).await
    }

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
