//! A run read that needs an evidence permission, authorized and audited in one transaction (once per `repeat` window when given), for other domains.

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
        (
            &str,
            impl AsyncFnOnce(
                ControlTransaction,
                &RunRecallScope,
            ) -> Result<(ControlTransaction, T), ControlError>,
            impl FnOnce(&T) -> Value,
        ),
        Option<Duration>,
    ),
) -> Result<Option<T>, ControlError> {
    sensitive_run_read(store, credentials, run, read).await
}
