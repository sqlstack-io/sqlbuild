//! Run directory confinement and decoding of native cost artifacts.

use std::fs;
use std::path::{Path, PathBuf};

use sqlbuild_core::cost_report::errors::CostArtifactError;
use sqlbuild_core::cost_report::main::check_cost_run_id::check_cost_run_id;
use sqlbuild_core::cost_report::main::read_cost_artifact::read_cost_artifact;
use sqlbuild_core::cost_report::models::CostRun;
use sqlbuild_core::json::models::{JsonInteger, JsonValue};

use crate::cost_store::errors::CostStoreError;

pub(crate) const ARTIFACT_NAME: &str = "run.json";
pub(crate) const TEMPORARY_NAME: &str = ".run.json.tmp";

pub(crate) fn root(project_dir: &Path) -> PathBuf {
    project_dir.join("target").join("executions")
}

pub(crate) fn directory(project_dir: &Path, run_id: &str) -> Result<PathBuf, CostStoreError> {
    check_cost_run_id(run_id)?;
    let root = root(project_dir);
    let directory = root.join(run_id);
    if let (Ok(resolved), Ok(resolved_root)) = (directory.canonicalize(), root.canonicalize())
        && !resolved.starts_with(resolved_root)
    {
        return Err(CostArtifactError::new(
            "cost run path must remain under the project execution directory",
        )
        .into());
    }
    Ok(directory)
}

fn entry(
    (key, payload): (String, serde_json::Value),
) -> Result<(String, JsonValue), CostArtifactError> {
    Ok((key, value(payload)?))
}

fn value(value: serde_json::Value) -> Result<JsonValue, CostArtifactError> {
    Ok(match value {
        serde_json::Value::Null => JsonValue::Null,
        serde_json::Value::Bool(value) => JsonValue::Bool(value),
        serde_json::Value::String(value) => JsonValue::String(value),
        serde_json::Value::Number(value) => {
            if let Some(integer) = JsonInteger::parse(&value.to_string()) {
                JsonValue::Integer(integer)
            } else {
                JsonValue::Float(value.as_f64().ok_or_else(|| {
                    CostArtifactError::new("cost run artifact number is out of range")
                })?)
            }
        }
        serde_json::Value::Array(values) => JsonValue::Array(
            values
                .into_iter()
                .map(self::value)
                .collect::<Result<_, _>>()?,
        ),
        serde_json::Value::Object(values) => {
            JsonValue::Object(values.into_iter().map(entry).collect::<Result<_, _>>()?)
        }
    })
}

pub(crate) fn load(path: &Path) -> Result<CostRun, CostStoreError> {
    let bytes = fs::read(path)?;
    let text = String::from_utf8(bytes)
        .map_err(|_| CostArtifactError::new("cost run artifact is not UTF-8 text"))?;
    let payload = serde_json::from_str(&text)
        .map_err(|_| CostArtifactError::new("cost run artifact is not valid JSON"))?;
    Ok(read_cost_artifact(&value(payload)?)?)
}
