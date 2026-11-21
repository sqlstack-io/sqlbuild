//! How much each node of a run printed, what its nodes logged, and one node's error and warning lines; output can quote warehouse data, so it needs view.evidence.

use serde_json::json;

use crate::credentials::models::Action;
use crate::run_output::_helpers::views::output_index;
use crate::run_output::models::RunOutputIndex;
use crate::run_output::types::PersonRunRead;
use crate::runs::constants::REPEATED_READ_AUDIT;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::main::read_run_evidence::read_run_evidence;

/// `None` when the run is not in this location and target; `node` names whose marks to include; each read is audited.
pub async fn read_person_output_index(
    store: &ControlStore,
    (credentials, run, default_role): PersonRunRead<'_>,
    node: Option<&str>,
) -> Result<Option<RunOutputIndex>, ControlError> {
    let run_id = run.2;
    read_run_evidence(
        store,
        credentials,
        (run, default_role, Action::ViewEvidence),
        (
            "output.index",
            async move |transaction, _| output_index(transaction, (run_id, node)).await,
            |index: &RunOutputIndex| json!({"nodes": index.nodes.len(), "logs": index.logs.len(), "node": node}),
        ),
        Some(REPEATED_READ_AUDIT),
    )
    .await
}
