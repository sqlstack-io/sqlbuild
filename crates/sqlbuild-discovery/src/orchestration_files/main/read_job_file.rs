//! Read one job file.

use std::collections::BTreeMap;

use crate::orchestration_files::_helpers::raw::{RawJob, RawRun};
use crate::orchestration_files::_helpers::reading::{
    check_keys, nonblank, position, positioned, run_file, toml_error,
};
use crate::orchestration_files::constants::JOB_KEYS;
use crate::orchestration_files::models::{FileError, FilePosition, JobFile, Positioned};

/// The job `text` declares, or the first problem with it and where it is.
pub fn read_job_file(text: &str) -> Result<JobFile, FileError> {
    check_keys(text, &JOB_KEYS, "job")?;
    let run: RawRun = toml::from_str(text).map_err(|error| toml_error(text, &error))?;
    let raw: RawJob = toml::from_str(text).map_err(|error| toml_error(text, &error))?;
    let resources = raw.resources.map(|resources| {
        let start: usize = resources.span().start;
        let table = serde_json::to_value(resources.into_inner())
            .ok()
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        Positioned {
            value: table,
            position: position(text, start),
        }
    });
    let mut settings: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut setting_positions: BTreeMap<String, FilePosition> = BTreeMap::new();
    for (key, value) in raw.settings {
        let value = positioned(text, value);
        settings.insert(
            key.clone(),
            serde_json::to_value(value.value).unwrap_or_default(),
        );
        setting_positions.insert(key, value.position);
    }
    Ok(JobFile {
        run: run_file(text, run)?,
        select: nonblank(text, raw.select, "select")?,
        exclude: raw
            .exclude
            .filter(|exclude| !exclude.get_ref().trim().is_empty())
            .map(|exclude| positioned(text, exclude)),
        resources,
        settings,
        setting_positions,
    })
}
