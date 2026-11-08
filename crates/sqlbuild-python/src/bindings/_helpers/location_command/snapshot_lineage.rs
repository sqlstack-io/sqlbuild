//! The project's column lineage edges, published so people can trace columns without the project.

use pyo3::prelude::{Bound, PyAny, PyResult};
use serde_json::{Value, json};

use crate::bindings::_helpers::lineage::column_lineage_graph::ProjectColumnLineage;
use crate::bindings::_helpers::lineage::project_lineage::build_project_column_lineage;
use crate::bindings::_helpers::records::construct::enum_member;
use crate::bindings::_helpers::records::ids::RecordId;

/// FAST column lineage of every model as `[source key, column, target key, column, transform]` rows; `None` without SQL analysis.
pub(crate) fn column_lineage_document(project: &Bound<'_, PyAny>) -> PyResult<Option<Value>> {
    let py = project.py();
    let mode = enum_member(py, RecordId::ColumnLineageMode, "FAST")?;
    let Some(lineage) = build_project_column_lineage(project, None, &mode, None)? else {
        return Ok(None);
    };
    let keys = lineage
        .bind(py)
        .cast::<ProjectColumnLineage>()?
        .borrow_mut()
        .edge_keys(py)?;
    let rows: Vec<Value> = keys
        .into_iter()
        .map(|key| {
            json!([
                format!("{}:{}", key.source_type, key.source_name),
                key.source_column,
                format!("model:{}", key.target_model),
                key.target_column,
                key.transform,
            ])
        })
        .collect();
    Ok(Some(json!({ "edges": rows })))
}
