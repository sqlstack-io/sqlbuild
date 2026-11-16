//! Direct non-projection semantic column uses of the requested models.

use crate::lineage::_helpers::graph::project_lineage::GENERIC_DIALECT;
use crate::lineage::_helpers::graph::project_lineage::semantic_use;
use crate::lineage::_helpers::graph::project_schema::rich_schema;
use crate::lineage::models::LineageClasses;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_analysis::lineage::main::direct_semantic_uses::direct_semantic_uses;
use sqlbuild_analysis::lineage::models::DirectSemanticUse;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::types::CompilerDetach;

/// Direct non-projection semantic column uses of the requested models.
#[pyfunction]
pub fn build_direct_semantic_uses<'py>(
    project: &Bound<'py, PyAny>,
    dialect: Option<&str>,
    model_names: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project.py();
    let classes: LineageClasses<'py> = LineageClasses::new(py)?;
    let schema = rich_schema(project)?;
    let mut models: Vec<(String, String, String)> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let name: Bound<'py, PyAny> = native_field(&model, "name")?;
        if model_names.contains(&name)? {
            models.push((
                name.extract()?,
                native_field(&model, "query_sql")?.extract()?,
                native_field(&model, "authored_sql")?.extract()?,
            ));
        }
    }
    let dialect: &str = dialect
        .filter(|name| !name.is_empty())
        .unwrap_or(GENERIC_DIALECT);
    let uses: Vec<DirectSemanticUse> = py
        .compiler_detach(|| direct_semantic_uses(&schema, &models, dialect))
        .map_err(compiler_error)?;
    let objects: Vec<Bound<'py, PyAny>> = uses
        .into_iter()
        .map(|usage| semantic_use(&classes, usage))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, objects)
}
