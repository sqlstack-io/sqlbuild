//! The project's column lineage edges, published so people can trace columns without the project.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use serde_json::{Value, json};

use crate::bindings::_helpers::lineage::project_lineage::build_project_column_lineage;
use crate::bindings::_helpers::records::construct::enum_member;
use crate::bindings::_helpers::records::fields::field;
use crate::bindings::_helpers::records::ids::RecordId;

/// FAST column lineage of every model as `[source key, column, target key, column, transform]` rows; `None` without SQL analysis.
pub(crate) fn column_lineage_document(project: &Bound<'_, PyAny>) -> PyResult<Option<Value>> {
    let py = project.py();
    let mode = enum_member(py, RecordId::ColumnLineageMode, "FAST")?;
    let Some(lineage) = build_project_column_lineage(project, None, &mode, None)? else {
        return Ok(None);
    };
    let edges = field(lineage.bind(py), "edges")?;
    let mut rows: Vec<Value> = Vec::new();
    for edge in edges.try_iter()? {
        let edge = edge?;
        let (source, target) = (field(&edge, "source")?, field(&edge, "target")?);
        rows.push(json!([
            node_key(&source)?,
            field(&source, "column_name")?.extract::<String>()?,
            node_key(&target)?,
            field(&target, "column_name")?.extract::<String>()?,
            field(&edge, "transform_kind")?.str()?.extract::<String>()?,
        ]));
    }
    Ok(Some(json!({ "edges": rows })))
}

/// The snapshot node key of a lineage column's resource, such as `model:orders`.
fn node_key(column: &Bound<'_, PyAny>) -> PyResult<String> {
    let kind: String = field(column, "resource_type")?.str()?.extract()?;
    let name: String = field(column, "resource_name")?.extract()?;
    Ok(format!("{kind}:{name}"))
}
