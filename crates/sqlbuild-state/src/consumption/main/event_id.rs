//! A consumption row's identity: its execution, phase and the input it records.

use sha2::{Digest, Sha256};

use crate::consumption::constants::EVENT_NAMESPACE;
use crate::consumption::models::ConsumedInput;

/// SHA-256 of `(execution_id, phase, input_kind, input_name, input_version)`, so a retried append repeats it.
pub fn event_id(execution_id: &str, phase: &str, input: Option<&ConsumedInput>) -> String {
    let identity = serde_json::json!([
        EVENT_NAMESPACE,
        execution_id,
        phase,
        input.map(|input| input.kind.as_str()),
        input.map(|input| input.name.as_str()),
        input.map(|input| input.version.as_str()),
    ]);
    format!("{:x}", Sha256::digest(identity.to_string().as_bytes()))
}
