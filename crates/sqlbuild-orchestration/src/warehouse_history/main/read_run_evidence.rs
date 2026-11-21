//! A run read that needs an evidence permission, authorized and audited in one transaction, for other domains' run reads.
//! A read a page repeats while following a run passes `repeat`, and is audited once per that window.

use std::time::Duration;

use serde_json::Value;
use uuid::Uuid;

use crate::credentials::models::{Action, AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::models::{ControlStore, ControlTransaction};
use crate::warehouse_history::_helpers::run_recall::{RunRecallScope, sensitive_run_read};

/// `None` when the run is not in the location and target, as `sensitive_run_read` decides.
pub(crate) async fn read_run_evidence<T>(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    run: ((Uuid, &str, Uuid), DefaultRole, Action),
    read: (
        &str,
        impl AsyncFnOnce(
            ControlTransaction,
            &RunRecallScope,
        ) -> Result<(ControlTransaction, T), ControlError>,
        impl FnOnce(&T) -> Value,
    ),
    repeat: Option<Duration>,
) -> Result<Option<T>, ControlError> {
    sensitive_run_read(store, credentials, run, read, repeat).await
}
