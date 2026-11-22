//! The plan JSON's `cant_run` section.

use serde_json::{Value, json};

use crate::provider_check::models::SecretBlock;

/// Each provider, its unset variables, the nodes and sinks that use it and the nodes that read them; never a value.
pub fn cant_run_json(blocks: &[SecretBlock], env_file: Option<&str>) -> Value {
    Value::Array(
        blocks
            .iter()
            .map(|block| {
                json!({
                    "reason": "missing_secret",
                    "provider": block.provider,
                    "path": block.path,
                    "variables": block.variables,
                    "nodes": block.users,
                    "readers": block.readers,
                    "sinks": block.sinks,
                    "env_file": env_file,
                })
            })
            .collect(),
    )
}
