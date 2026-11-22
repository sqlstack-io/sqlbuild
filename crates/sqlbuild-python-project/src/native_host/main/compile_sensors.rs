//! Each decorated sensor's published definition: its function, the code it hashes to, and its resolved condition.

use crate::graph::models::ProjectSelection;
use crate::native_host::_helpers::orchestration_triggers::sensor_nodes;
use crate::native_host::constants::PYTHON_CHECK_KIND;
use crate::project_snapshot::main::attached_nodes::attached_nodes;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyAnyMethods, PyResult, Python};
use serde_json::Value;
use serde_json::json;
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_orchestration::declarations::models::Definitions;
use sqlbuild_orchestration::sensors::main::sensor_timeout_error::sensor_timeout_error;
use sqlbuild_orchestration::sensors::models::SensorDefinition;
use sqlbuild_orchestration::snapshots::models::SnapshotInput;
use sqlbuild_python_base::native_host::constants::CHECK_EVENT_CONDITION;
use sqlbuild_python_base::native_host::constants::SOURCE_NEW_DATA_CONDITION;
use sqlbuild_python_base::native_host::constants::SOURCE_STALE_CONDITION;
use sqlbuild_python_base::orchestration_reads::main::callback_code_hash::callback_code_hash;
use sqlbuild_python_base::orchestration_reads::main::declared_at::declared_at;
use sqlbuild_python_base::orchestration_reads::main::orchestration_code_hash::orchestration_code_hash;
use sqlbuild_python_base::orchestration_reads::models::{AuthoredSensor, AuthoredSensorCondition};
use std::collections::BTreeSet;
use std::path::Path;

/// The published definition of each decorated sensor the target publishes, keyed by name.
pub fn compile_sensors(
    py: Python<'_>,
    (project_dir, input, graph): (&Path, &SnapshotInput, (&ProjectGraph, &ProjectSelection)),
    authored: Vec<AuthoredSensor>,
    mut definitions: Definitions,
) -> PyResult<Definitions> {
    let sources = if authored.is_empty() {
        String::new()
    } else {
        orchestration_code_hash(project_dir)?
    };
    for sensor in authored {
        if !sensor.targets.is_empty() && !sensor.targets.iter().any(|value| value == &input.target)
        {
            continue;
        }
        let (Some(name), Some(function)) = (sensor.name, sensor.function) else {
            return Err(PyValueError::new_err(
                "sensor(...) must decorate a function: write @sensor(description=..., when=...) above a def",
            ));
        };
        let function = function.bind(py);
        let when: Option<Value> = sensor
            .when
            .map(|when| condition(input, graph, when))
            .transpose()?;
        let definition: SensorDefinition = serde_json::from_value(json!({
            "description": sensor.description,
            "when": when,
            "function": function.getattr("__name__")?.extract::<String>()?,
            "code_hash": callback_code_hash(function, &sources)?,
            "minimum_interval_seconds": sensor.minimum_interval_seconds,
            "timeout_seconds": sensor.timeout_seconds,
            "default_status": sensor.default_status,
            "targets": sensor.targets,
            "declared_at": declared_at(project_dir, sensor.declared_at)?,
        }))
        .map_err(|error| PyValueError::new_err(format!("sensor {name}: {error}")))?;
        if let Some(error) = sensor_timeout_error(&name, &definition) {
            return Err(PyValueError::new_err(error));
        }
        definitions.sensors.insert(name, definition);
    }
    Ok(definitions)
}

/// The published condition of `when`, its selectors resolved to node keys.
fn condition(
    input: &SnapshotInput,
    graph: (&ProjectGraph, &ProjectSelection),
    when: AuthoredSensorCondition,
) -> PyResult<Value> {
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
        let selectors: Vec<String> = select.split_whitespace().map(str::to_owned).collect();
        let nodes = sensor_nodes(graph, &selectors, &when.kind)?;
        condition["selection"] = if when.kind == CHECK_EVENT_CONDITION {
            json!(checks_of(input, &nodes)?)
        } else {
            nodes
        };
    }
    if let Some(selectors) = when.executed_successfully {
        condition["executed_successfully"] =
            sensor_nodes(graph, &selectors, "executed_successfully")?;
    }
    Ok(condition)
}

/// The checks, audits and SQL tests that run on `nodes`, and any of them `nodes` selects directly.
fn checks_of(input: &SnapshotInput, nodes: &Value) -> PyResult<BTreeSet<String>> {
    let selected: BTreeSet<&str> = nodes
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut checks: BTreeSet<String> = BTreeSet::new();
    for (key, checked) in attached_nodes(&input.nodes) {
        if selected.contains(key.as_str())
            || checked.iter().any(|node| selected.contains(node.as_str()))
        {
            checks.insert(key);
        }
    }
    for (key, node) in &input.nodes {
        let deps = node.definition.get("deps").and_then(Value::as_array);
        let checks_selected = deps
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .any(|dep| selected.contains(dep));
        if node.kind == PYTHON_CHECK_KIND && (selected.contains(key.as_str()) || checks_selected) {
            checks.insert(key.clone());
        }
    }
    if checks.is_empty() {
        return Err(PyValueError::new_err(
            "check_event selects nodes with no checks, audits or SQL tests",
        ));
    }
    Ok(checks)
}
