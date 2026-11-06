//! The code location has only its scoped HTTP credential, never a store handle.

use uuid::Uuid;

use crate::api::errors::ClientError;
use crate::platform::main::connect_location_work::connect_location_work;
use crate::platform::models::LocationHealthReport;

pub async fn report_remote_location_health(
    (server_url, bearer): (&str, &str),
    location_id: Uuid,
    report: &LocationHealthReport,
) -> Result<bool, ClientError> {
    connect_location_work((server_url, bearer), location_id)?
        .report_health(report)
        .await
}
