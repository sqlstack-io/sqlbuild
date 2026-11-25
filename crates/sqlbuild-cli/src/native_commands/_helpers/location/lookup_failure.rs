//! A location that checks in but can't get its next generation number can't load new code; at the third failed lookup in a row it reports a failed load.

use std::future::Future;
use std::time::Duration;

use sqlbuild_core::redaction::main::redact_error_text::redact_error_text;
use sqlbuild_orchestration::platform::models::LocationHealth;
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::time::Instant;
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::report_location_health;
use crate::native_commands::constants::MAX_LOAD_ERROR_CHARS;
use crate::native_commands::models::{GenerationLookups, LocationServe, LocationWatch};

/// Retry the lookup in 5 seconds and, at the third failure in a row, report a failed load with `error`; `None` when stopped first.
pub(crate) async fn failed_lookup(
    options: &LocationServe,
    (bearer, session_id): (&str, Uuid),
    (error, mut watch): (&str, LocationWatch),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<LocationWatch>, ControlError> {
    watch.retry_at = Some(Instant::now() + Duration::from_secs(5));
    if !watch.lookups.failed() {
        return Ok(Some(watch));
    }
    let error: String = redact_error_text(&format!(
        "the location can't get its next generation number from the server: {error}"
    ))
    .chars()
    .take(MAX_LOAD_ERROR_CHARS)
    .collect();
    drop(write_output(&[(
        Stream::Stderr,
        &format!("Reporting a failed load: {error}\n"),
    )]));
    let failed: LocationHealth = lookup_failure_health(watch.lookups, error);
    tokio::select! {
        result = stopped => { result?; Ok(None) },
        () = report_location_health(options, bearer, session_id, failed) => Ok(Some(watch)),
    }
}

/// A failed load of the generation the location can't get, with `error`.
pub(crate) fn lookup_failure_health(lookups: GenerationLookups, error: String) -> LocationHealth {
    LocationHealth::CompileFailed {
        generation: lookups.next(),
        rules: None,
        error: Some(error),
    }
}
