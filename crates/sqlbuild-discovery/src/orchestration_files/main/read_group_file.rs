//! Read one job group file.

use std::collections::BTreeSet;

use crate::orchestration_files::_helpers::raw::{RawGroup, RawRun};
use crate::orchestration_files::_helpers::reading::{
    check_keys, error_at, nonblank, run_file, toml_error,
};
use crate::orchestration_files::constants::GROUP_KEYS;
use crate::orchestration_files::models::{FileError, GroupFile, Positioned};

/// The job group `text` declares, or the first problem with it and where it is.
pub fn read_group_file(text: &str) -> Result<GroupFile, FileError> {
    check_keys(text, &GROUP_KEYS, "job group")?;
    let run: RawRun = toml::from_str(text).map_err(|error| toml_error(text, &error))?;
    let raw: RawGroup = toml::from_str(text).map_err(|error| toml_error(text, &error))?;
    if raw.jobs.is_empty() {
        return Err(error_at(
            text,
            run.name.span().start,
            "a job group needs at least one job in `jobs`".to_owned(),
        ));
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut jobs: Vec<Positioned<String>> = Vec::new();
    for job in raw.jobs {
        let job: Positioned<String> = nonblank(text, job, "job name")?;
        if !seen.insert(job.value.clone()) {
            return Err(FileError {
                message: format!("job '{}' is listed twice", job.value),
                position: job.position,
                help: None,
            });
        }
        jobs.push(job);
    }
    Ok(GroupFile {
        run: run_file(text, run)?,
        jobs,
    })
}
