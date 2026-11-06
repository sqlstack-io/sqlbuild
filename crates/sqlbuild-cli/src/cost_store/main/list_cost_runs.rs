//! Enumerate readable, confined run artifacts, newest first.

use crate::cost_store::_helpers::artifacts::{ARTIFACT_NAME, load, root};
use sqlbuild_core::cost_report::main::order_cost_runs::order_cost_runs;
use sqlbuild_core::cost_report::models::CostRun;
use std::fs;
use std::path::Path;

/// List readable artifacts; corrupt records, mismatched IDs and escaping symlinks are skipped.
pub fn list_cost_runs(project_dir: &Path) -> Vec<CostRun> {
    let root = root(project_dir);
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut runs: Vec<CostRun> = Vec::new();
    for entry in entries.flatten() {
        let directory = entry.path();
        let path = directory.join(ARTIFACT_NAME);
        let (Ok(resolved), Ok(resolved_root)) = (path.canonicalize(), root.canonicalize()) else {
            continue;
        };
        if !resolved.starts_with(resolved_root) || !path.is_file() {
            continue;
        }
        let Ok(run) = load(&path) else {
            continue;
        };
        if directory.file_name().and_then(|name| name.to_str()) == Some(run.run_id.as_str()) {
            runs.push(run);
        }
    }
    order_cost_runs(runs)
}
