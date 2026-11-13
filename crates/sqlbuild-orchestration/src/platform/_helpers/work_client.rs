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
    MetadataBatch, MetadataKeyPage, MetadataKeyPageResponse, PreviewCopy, PreviewCopyRead,
    StatisticsDueRead, StatisticsRead, WarehouseSyncDue, WarehouseSyncDueRead,
    WarehouseSyncResponse, WarehouseWatermarkRead,
};

impl LocationWorkClient {
    pub async fn statistics_due(
        &self,
        target: &str,
        request: &StatisticsDueRead,
    ) -> Result<StatisticsRead, ClientError> {
        self.post_metadata((target, Some("statistics-due")), request)
            .await
    }

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
        self.post_metadata((target, Some("watermark")), request)
            .await
    }

    pub async fn metadata_sync_due(
        &self,
        target: &str,
        request: &WarehouseSyncDueRead,
    ) -> Result<WarehouseSyncDue, ClientError> {
        self.post_metadata((target, Some("sync-due")), request)
            .await
    }

    /// The mirrored copy a plan preview plans from, so previewing never opens the warehouse.
    pub async fn preview_copy(
        &self,
        target: &str,
        request: &PreviewCopyRead,
    ) -> Result<PreviewCopy, ClientError> {
        self.post_metadata((target, Some("preview-copy")), request)
            .await
    }

    /// Metadata shares the location's scoped pool, not the scheduler's execution authority.
    pub async fn sync_metadata(
        &self,
        target: &str,
        batch: &MetadataBatch,
    ) -> Result<WarehouseSyncResponse, ClientError> {
        self.post_metadata((target, None), batch).await
    }

    /// Marks the scan's rows the mirror holds and returns the positions of those it lacks.
    pub async fn match_metadata_keys(
        &self,
        target: &str,
        page: &MetadataKeyPage,
    ) -> Result<MetadataKeyPageResponse, ClientError> {
        self.post_metadata((target, Some("keys")), page).await
    }

    async fn post_metadata<T: serde::de::DeserializeOwned>(
        &self,
        (target, read): (&str, Option<&str>),
        body: &impl serde::Serialize,
    ) -> Result<T, ClientError> {
        let location_id = self.location_id.to_string();
        let mut segments = vec![
            "api",
            "locations",
            &location_id,
            "targets",
            target,
            "metadata",
        ];
        segments.extend(read);
        let endpoint = self.connection.endpoint(&segments)?;
        client_response(self.connection.client.post(endpoint).json(body)).await
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
