//! Compile sensors.

use crate::graph::models::ProjectSelection;
use crate::native_host::_helpers::orchestration_triggers::sensor_nodes;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyResult, Python};
use serde_json::json;
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_orchestration::declarations::models::Definitions;
use sqlbuild_orchestration::sensors::models::SensorDefinition;
use sqlbuild_orchestration::snapshots::models::SnapshotInput;
use sqlbuild_python_base::native_host::constants::SOURCE_NEW_DATA_CONDITION;
use sqlbuild_python_base::native_host::constants::SOURCE_STALE_CONDITION;
use sqlbuild_python_base::orchestration_reads::main::callback_code_hash::callback_code_hash;
use sqlbuild_python_base::orchestration_reads::main::declared_at::declared_at;
use sqlbuild_python_base::orchestration_reads::main::orchestration_code_hash::orchestration_code_hash;
use sqlbuild_python_base::orchestration_reads::models::AuthoredSensor;
use std::path::Path;

/// Compile sensors.
pub fn compile_sensors(
    py: Python<'_>,
    (project_dir, input, graph): (&Path, &SnapshotInput, (&ProjectGraph, &ProjectSelection)),
    authored: Vec<AuthoredSensor>,
    mut definitions: Definitions,
) -> PyResult<Definitions> {
    let sources = if authored.iter().any(|sensor| sensor.function.is_some()) {
        orchestration_code_hash(project_dir)?
    } else {
        String::new()
    };
    for sensor in authored {
        if !sensor.targets.is_empty() && !sensor.targets.iter().any(|value| value == &input.target)
        {
            continue;
        }
        let condition = match (sensor.when, sensor.function) {
            (Some(when), None) => {
                let mut condition = json!({"kind": when.kind});
                if when.kind == SOURCE_STALE_CONDITION {
                    condition["after_seconds"] = when.after_seconds;
                } else if when.kind != SOURCE_NEW_DATA_CONDITION {
                    condition["kinds"] = json!(when.kinds);
                }
                if let Some(job) = when.job {
                    condition["job_key"] = json!(job);
                }
                if let Some(select) = when.select {
                    let selectors: Vec<String> =
                        select.split_whitespace().map(str::to_owned).collect();
                    condition["selection"] = sensor_nodes(graph, &selectors, &when.kind)?;
                }
                if let Some(selectors) = when.executed_successfully {
                    condition["executed_successfully"] =
                        sensor_nodes(graph, &selectors, "executed_successfully")?;
                }
                condition
            }
            (None, Some(function)) => json!({"kind": "custom", "function": sensor.name,
                "code_hash": callback_code_hash(function.bind(py), &sources)?}),
            _ => {
                return Err(PyValueError::new_err(
                    "sensor needs either a built-in condition or a decorated polling callback; callbacks with when are not supported yet",
                ));
            }
        };
        let definition: SensorDefinition = serde_json::from_value(json!({
            "description": sensor.description,
            "job_key": sensor.run,
            "when": condition,
            "minimum_interval_seconds": sensor.minimum_interval_seconds,
            "default_status": sensor.default_status,
            "targets": sensor.targets,
            "declared_at": declared_at(project_dir, sensor.declared_at)?,
        }))
        .map_err(|error| PyValueError::new_err(format!("sensor {}: {error}", sensor.name)))?;
        definitions.sensors.insert(sensor.name, definition);
    }
    Ok(definitions)
}
