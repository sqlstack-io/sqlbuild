//! Lifecycle changes share one transaction owner across trusted callers and person operations.

use crate::runs::_helpers::lifecycle::change_run::change_run_in;
use crate::runs::models::{RunAuthority, RunChangeRequest};
use crate::store::errors::ControlError;
use crate::store::main::write::write;
use crate::store::models::ControlStore;

pub async fn change_run(
    store: &ControlStore,
    authority: RunAuthority<'_>,
    request: &RunChangeRequest,
) -> Result<bool, ControlError> {
    write(store, |transaction| {
        change_run_in(transaction, authority, request)
    })
    .await
}
