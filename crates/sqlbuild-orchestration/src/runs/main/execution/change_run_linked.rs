//! A lifecycle change whose run failure links under the address people reach the UI at.

use crate::runs::_helpers::lifecycle::change_run::change_run_in;
use crate::runs::models::{RunAuthority, RunChangeRequest};
use crate::store::errors::ControlError;
use crate::store::main::write::write;
use crate::store::models::ControlStore;

/// A change whose run failure links under `origin`, the address people reach the UI at, as a worker's report through the API.
pub async fn change_run_linked(
    store: &ControlStore,
    (authority, request): (RunAuthority<'_>, &RunChangeRequest),
    origin: Option<&str>,
) -> Result<bool, ControlError> {
    write(store, |transaction| {
        change_run_in(transaction, (authority, request), origin)
    })
    .await
}
