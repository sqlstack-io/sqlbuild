//! A compiled project record's lineage graph: edges, tags, folders and names indexed natively.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

use crate::graph::_helpers::project::record_rows::{
    function_tags, model_tags, resource_rows, seed_tags, source_tags,
};
use crate::graph::main::project::load_rows::load_rows;
use crate::graph::main::project::project_graph::project_graph;
use crate::graph::models::ResourceRow;

/// `CompiledProject.lineage_graph`: lineage edges, tags, folders and names indexed natively.
pub fn record_lineage_graph(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let mut rows: Vec<ResourceRow> = Vec::new();
    rows.extend(resource_rows(
        &native_field(instance, "models")?,
        model_tags,
    )?);
    rows.extend(resource_rows(
        &native_field(instance, "sources")?,
        source_tags,
    )?);
    rows.extend(resource_rows(&native_field(instance, "seeds")?, seed_tags)?);
    rows.extend(resource_rows(
        &native_field(instance, "functions")?,
        function_tags,
    )?);
    rows.extend(load_rows(&native_field(instance, "loads")?)?);
    project_graph(rows)?.into_py_any(instance.py())
}
