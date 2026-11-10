//! Reading `sqb plan --json` output as the plan a location sends to the server.

use serde_json::{Value, json};
use sqlbuild_orchestration::platform::constants::PLAN_DOCUMENT_BYTES;

use crate::native_commands::constants::PLAN_OUTPUT_BYTES;

/// The plan `sqb plan --json` printed, without model diffs when they would put it over the server's cap.
pub(in crate::native_commands) fn plan_document(output: &[u8], planned: bool) -> Value {
    if output.len() > PLAN_OUTPUT_BYTES {
        return failure("the plan is too large to show");
    }
    let mut document = match serde_json::from_slice::<Value>(output) {
        Ok(document) if document.is_object() && planned => document,
        _ => return failure("sqb plan did not produce a plan; see the location's log"),
    };
    if output.len() <= PLAN_DOCUMENT_BYTES {
        return document;
    }
    let models = document.get_mut("models").and_then(Value::as_array_mut);
    for model in models
        .into_iter()
        .flatten()
        .filter_map(Value::as_object_mut)
    {
        model.remove("query_diff");
        model.remove("config_diff");
    }
    match serde_json::to_vec(&document) {
        Ok(bytes) if bytes.len() <= PLAN_DOCUMENT_BYTES => document,
        _ => failure("the plan is too large to show"),
    }
}

/// The `{"error": ...}` report a location sends when it has no plan to show.
pub(in crate::native_commands) fn failure(message: &str) -> Value {
    json!({"error": {"code": "plan_failed", "message": message, "help": null}})
}
