//! What a worker run reports when its selection needs secrets that are missing.

use std::path::Path;

use sqlbuild_python_front::discovery::main::sources::secret_places::secret_places;

use crate::provider_check::constants::SINK_KEY_PREFIX;
use crate::provider_check::models::SecretBlock;

/// Each selected node that uses a provider missing secrets, with its failure message, and the run's status reason.
pub fn missing_secret_failures(
    blocks: &[SecretBlock],
    project_dir: &Path,
) -> (Vec<(String, String)>, String) {
    let places: String =
        secret_places(project_dir).replacen("the environment", "the location's environment", 1);
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut needs: Vec<String> = Vec::new();
    for block in blocks {
        let variables: String = block.variables.join(", ");
        needs.push(format!("provider {} needs {variables}", block.provider));
        let message: String = format!(
            "provider {}: {variables} not set (missing from {places})",
            block.provider
        );
        for key in &block.users {
            if !key.starts_with(SINK_KEY_PREFIX) {
                failures.push((key.clone(), message.clone()));
            }
        }
    }
    let reason: String = format!(
        "Couldn't run: {} (missing from {places}); nothing ran",
        needs.join("; ")
    );
    (failures, reason)
}
