//! Python's `analyze_dynamic_column_contract` for models no analysis session analysed.

use sqlbuild_core::constants::PANIC_MESSAGE;

use crate::assembly::analysis_session::_helpers::dynamic_pivot::pivot_outcomes;
use crate::assembly::analysis_session::constants::PIVOT_WORKER_STACK_BYTES;
use crate::assembly::analysis_session::models::{PivotBatchRequest, PivotOutcome};
use crate::worker_stack::main::run_on_worker_stack::run_on_worker_stack;

/// Each model's proof in order; an internal failure of any proof fails them all.
pub fn prove_dynamic_contracts(request: &PivotBatchRequest) -> Result<Vec<PivotOutcome>, String> {
    run_on_worker_stack(PIVOT_WORKER_STACK_BYTES, PANIC_MESSAGE, || {
        pivot_outcomes(&request.models, &request.tables)
    })?
}
