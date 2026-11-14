//! Read one job file.

use std::collections::BTreeMap;

use toml::Spanned;

use crate::orchestration_files::_helpers::raw::{RawJob, RawRun};
use crate::orchestration_files::_helpers::reading::{
    check_keys, error_at, nonblank, position, positioned, run_file, toml_error,
};
use crate::orchestration_files::constants::JOB_KEYS;
use crate::orchestration_files::errors::FileError;
use crate::orchestration_files::models::{FilePosition, JobFile, Positioned};

/// The job `text` declares, or the first problem with it and where it is.
pub(crate) fn read_job_file(text: &str) -> Result<JobFile, FileError> {
    check_keys(text, &JOB_KEYS, "job")?;
    let run: RawRun = toml::from_str(text).map_err(|error| toml_error(text, &error))?;
    let raw: RawJob = toml::from_str(text).map_err(|error| toml_error(text, &error))?;
    let resources: Option<Positioned<serde_json::Map<String, serde_json::Value>>> = raw
        .resources
        .map(|resources| json_table(text, resources))
        .transpose()?;
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

/// A TOML table as JSON, positioned where it is written.
fn json_table(
    text: &str,
    table: Spanned<toml::Table>,
) -> Result<Positioned<serde_json::Map<String, serde_json::Value>>, FileError> {
    let start: usize = table.span().start;
    match serde_json::to_value(table.into_inner()) {
        Ok(serde_json::Value::Object(value)) => Ok(Positioned {
            value,
            position: position(text, start),
        }),
        Ok(_) | Err(_) => Err(error_at(
            text,
            start,
            "resources must be a table of plain values".to_owned(),
        )),
    }
}
