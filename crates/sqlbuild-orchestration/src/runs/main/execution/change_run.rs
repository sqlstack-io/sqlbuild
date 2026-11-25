//! Lifecycle changes share one transaction owner across trusted callers and person operations.

use crate::runs::main::execution::change_run_linked::change_run_linked;
use crate::runs::models::{RunAuthority, RunChangeRequest};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// A change made under the scheduler lease links a failed run under the lease's origin; others link nothing.
pub async fn change_run(
    store: &ControlStore,
    authority: RunAuthority<'_>,
    request: &RunChangeRequest,
) -> Result<bool, ControlError> {
    change_run_linked(store, (authority, request), None).await
}
