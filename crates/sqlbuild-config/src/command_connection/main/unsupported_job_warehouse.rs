//! A job `warehouse` for an adapter without a session warehouse.

use crate::command_connection::constants::UNSUPPORTED_WAREHOUSE_CODE;
use crate::command_connection::errors::CommandConnectionError;

/// The error for a job's `warehouse` the adapter cannot select.
#[must_use]
pub fn unsupported_job_warehouse(
    job: &str,
    warehouse: &str,
    adapter_name: &str,
) -> CommandConnectionError {
    CommandConnectionError::User {
        code: Some(UNSUPPORTED_WAREHOUSE_CODE),
        message: format!(
            "job {job} sets warehouse {warehouse}, but the '{adapter_name}' adapter has no warehouse to select"
        ),
        help: Some(
            "remove warehouse from the job's [settings], or build with --no-job-settings; it needs an adapter with a session warehouse, such as snowflake or an adapter extending it"
                .to_owned(),
        ),
    }
}
