//! Reading `sqb plan --json` output as the plan a location sends to the server.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sqlbuild_core::plan_display::main::detach_model_diffs::detach_model_diffs;
use sqlbuild_orchestration::platform::constants::{PLAN_DIFF_BYTES, PLAN_DOCUMENT_BYTES};
use sqlbuild_orchestration::platform::models::PlanModelDiff;

use crate::native_commands::_helpers::location::execution::{plan_failure, without_diffs};
use crate::native_commands::constants::{PLAN_NOT_PRODUCED, PLAN_OUTPUT_BYTES};
use crate::native_commands::types::LocationPlan;

/// The plan `sqb plan --json` printed; its model diffs leave it, keeping their counts, and travel beside it.
pub(in crate::native_commands) fn plan_document(output: &[u8], planned: bool) -> LocationPlan {
    if output.len() > PLAN_OUTPUT_BYTES {
        return without_diffs(plan_failure("the plan is too large to show"));
    }
    let document = match serde_json::from_slice::<Value>(output) {
        Ok(document) if document.is_object() && planned => document,
        _ => return without_diffs(plan_failure(PLAN_NOT_PRODUCED)),
    };
    let (document, detached) = detach_model_diffs(document);
    if !serde_json::to_vec(&document).is_ok_and(|bytes| bytes.len() <= PLAN_DOCUMENT_BYTES) {
        return without_diffs(plan_failure("the plan is too large to show"));
    }
    match serde_json::from_value::<BTreeMap<String, PlanModelDiff>>(json!(detached)) {
        Ok(diffs)
            if serde_json::to_vec(&diffs).is_ok_and(|bytes| bytes.len() <= PLAN_DIFF_BYTES) =>
        {
            (document, diffs)
        }
        _ => without_diffs(document),
    }
}
