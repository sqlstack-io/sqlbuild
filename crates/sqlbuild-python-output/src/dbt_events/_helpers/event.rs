//! The fields of one dbt JSON log line, read with Python `dict.get` semantics.

use pyo3::PyResult;
use pyo3::exceptions::PyRuntimeError;
use serde_json::{Map, Value};
use sqlbuild_core::dbt_events::models::{DbtEvent, DbtEventNode};
use sqlbuild_core::text::main::python_strip::python_strip;

static EMPTY: std::sync::LazyLock<Map<String, Value>> = std::sync::LazyLock::new(Map::new);

/// The object under `key`, or an empty one.
fn object<'a>(parent: &'a Map<String, Value>, key: &str) -> &'a Map<String, Value> {
    parent.get(key).and_then(Value::as_object).unwrap_or(&EMPTY)
}

fn text(parent: &Map<String, Value>, key: &str) -> Option<String> {
    parent.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn integer(parent: &Map<String, Value>, key: &str) -> Option<i64> {
    parent.get(key).and_then(Value::as_i64)
}

fn node(data: &Map<String, Value>) -> DbtEventNode {
    let info: &Map<String, Value> = object(data, "node_info");
    let relation: &Map<String, Value> = object(info, "node_relation");
    DbtEventNode {
        unique_id: text(info, "unique_id"),
        resource_type: text(info, "resource_type"),
        node_name: text(info, "node_name"),
        materialized: text(info, "materialized"),
        node_status: text(info, "node_status"),
        node_checksum: text(info, "node_checksum"),
        relation_name: text(relation, "relation_name"),
        database: text(relation, "database"),
        schema: text(relation, "schema"),
    }
}

/// The event on one output line, or `None` for non-JSON output and non-object JSON.
pub(crate) fn dbt_event(line: &str) -> PyResult<Option<DbtEvent>> {
    let stripped: &str = python_strip(line);
    if !stripped.starts_with('{') {
        return Ok(None);
    }
    let payload: Value = match serde_json::from_str(stripped) {
        Ok(payload) => payload,
        Err(error) if error.is_syntax() || error.is_eof() || error.is_data() => return Ok(None),
        Err(error) => return Err(PyRuntimeError::new_err(error.to_string())),
    };
    let Some(event) = payload.as_object() else {
        return Ok(None);
    };
    let info: &Map<String, Value> = object(event, "info");
    let data: &Map<String, Value> = object(event, "data");
    Ok(Some(DbtEvent {
        name: text(info, "name"),
        level: text(info, "level"),
        info_msg: text(info, "msg"),
        data_msg: text(data, "msg"),
        data_status: text(data, "status"),
        run_result_status: text(object(data, "run_result"), "status"),
        index: integer(data, "index"),
        total: integer(data, "total"),
        num_models: integer(data, "num_models"),
        execution_time: data.get("execution_time").and_then(Value::as_f64),
        node: node(data),
    }))
}
