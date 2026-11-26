//! Where a command's run selectors read past runs: the server it names, else the local SQLite Recall history.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::native_commands::main::effective_target::effective_target;
use crate::run_selectors::models::PastRunSource;

/// The `--server` with its `--ca-cert` and `--location-id` on the project's target, else the local `recall_store`; why neither works when that's so.
#[must_use]
pub fn past_run_source(
    (server, ca, location): (Option<String>, Option<PathBuf>, Option<String>),
    (project_dir, selected_target): (&Path, Option<String>),
    recall_store: Option<PathBuf>,
) -> PastRunSource {
    let unavailable = |reason: &str| PastRunSource::Unavailable(reason.to_owned());
    let Some(origin) = server else {
        return recall_store.map_or_else(
            || {
                unavailable(
                    "run selectors read the local Recall history only from a SQLite store; pass --server to read the server's runs",
                )
            },
            |store| PastRunSource::Recall { store },
        );
    };
    let Some(location_id) = location.as_deref().map(Uuid::parse_str) else {
        return unavailable("--location-id is required with --server");
    };
    let Ok(location_id) = location_id else {
        return unavailable("--location-id must be a UUID");
    };
    match effective_target(project_dir, selected_target) {
        Ok(Some(target)) => PastRunSource::Server {
            origin,
            ca,
            location_id,
            target,
        },
        Ok(None) => unavailable("--server needs a target; pass --target"),
        Err(error) => PastRunSource::Unavailable(error),
    }
}
