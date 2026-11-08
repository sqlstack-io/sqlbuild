//! Run output and error text can quote warehouse data, so they need view.evidence on every node the run selected.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::{Action, AuditContext, DefaultRole};
use crate::runs::models::RunOutput;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::_helpers::run_recall::{run_output_in, sensitive_run_read};

/// `None` when the run is not in this location and target; each read is audited like check evidence.
pub async fn read_person_run_output(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    run: (Uuid, &str, Uuid),
    default_role: DefaultRole,
) -> Result<Option<RunOutput>, ControlError> {
    sensitive_run_read(
        store,
        credentials,
        (run, default_role, Action::ViewEvidence),
        (
            "output.read",
            run_output_in,
            |output: &RunOutput| json!({"lines": output.lines.len(), "errors": output.errors.len()}),
        ),
    )
    .await
}
