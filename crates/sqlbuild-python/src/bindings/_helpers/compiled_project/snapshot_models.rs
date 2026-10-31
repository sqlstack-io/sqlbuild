//! Model catalog data comes directly from the compiler's retained facts and records.

use std::collections::BTreeMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyResult, Python};
use pyo3::types::PyTupleMethods;
use serde_json::{Value, json};
use sqlbuild_analysis::compiled_project::main::model_facts::model_facts;
use sqlbuild_orchestration::snapshots::models::SnapshotNode;

use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_project::snapshot::{
    dependency_keys, destination, insert_node,
};
use crate::bindings::_helpers::compiled_views::compiled_model_view::NativeCompiledModel;
use crate::bindings::_helpers::compiled_views::project_view::NativeProjectView;

pub(crate) fn model_nodes(
    py: Python<'_>,
    project: &NativeProjectView,
    owner: &NativeCompiledProject,
    mut nodes: BTreeMap<String, SnapshotNode>,
) -> PyResult<BTreeMap<String, SnapshotNode>> {
    for model in project.models(py).bind(py).iter() {
        let model = model.cast::<NativeCompiledModel>()?.get();
        let (name, _, _) = model.file_texts(py)?;
        let facts = model_facts(owner.facts(), &name)
            .ok_or_else(|| PyValueError::new_err("snapshot model has no retained facts"))?;
        if facts.config.encoding_error.is_some() {
            return Err(PyValueError::new_err(
                "model config cannot be represented in snapshot metadata",
            ));
        }
        let mut definition = model.read(py, |input, _| {
            let record = input
                .compiled
                .as_ref()
                .ok_or_else(|| PyValueError::new_err("snapshot model has not been assembled"))?;
            let columns: Vec<Value> = record
                .analysis
                .inferred
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|(name, data_type, nullability)| {
                    json!({"name": name, "type": data_type, "nullability": nullability})
                })
                .collect();
            Ok(json!({
                "name": facts.name,
                "path": facts.relative_path,
                "config": facts.config.values,
                "materialization": facts.config.materialized,
                "deps": dependency_keys(&record.deps),
                "columns": columns,
            }))
        })?;
        definition["destination"] = model.read_destination(py, |value| Ok(destination(value)))?;
        nodes = insert_node(
            nodes,
            format!("model:{}", facts.name),
            SnapshotNode {
                kind: "model".into(),
                definition,
                raw_sql: Some(facts.authored_sql.to_string()),
                compiled_sql: Some(facts.query_sql.to_string()),
                python_source: None,
            },
        )?;
    }
    Ok(nodes)
}
