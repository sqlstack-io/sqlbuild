//! The discovered Python nodes by kind, name and path only, as the resource-name check reads them.

use crate::discovery::_helpers::project::project_facts::PYTHON_NODE_FIELDS;
use crate::discovery::_helpers::project::project_facts::each;
use crate::discovery::_helpers::project::project_facts::text;
use crate::discovery::models::NodeRow;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The discovered Python nodes by kind, name and path only, as the resource-name check reads them.
pub fn named_node_rows(inputs: &Bound<'_, PyAny>) -> PyResult<Vec<NodeRow>> {
    let mut rows: Vec<NodeRow> = Vec::new();
    for (kind, name) in PYTHON_NODE_FIELDS {
        rows.extend(each(inputs, name, |node| {
            Ok(NodeRow {
                kind: kind.to_owned(),
                name: native_field(node, "name")?.extract()?,
                path: text(node, "relative_path")?,
                decorator_config: false,
                depends_on: Vec::new(),
            })
        })?);
    }
    Ok(rows)
}
