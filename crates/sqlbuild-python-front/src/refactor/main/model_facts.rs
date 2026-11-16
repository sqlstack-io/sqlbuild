//! Every compiled model's refactoring facts, or none without a project.

use crate::refactor::_helpers::refactor_facts::model;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_refactor::refactoring::models::ModelFacts;
use std::collections::HashMap;

/// Every compiled model's refactoring facts, or none without a project.
pub fn model_facts(refactor_project: &Bound<'_, PyAny>) -> PyResult<Option<Vec<ModelFacts>>> {
    if refactor_project.is_none() {
        return Ok(None);
    }
    let project: Bound<'_, PyAny> =
        native_field(&native_field(refactor_project, "graph")?, "project")?;
    let project_dir: Bound<'_, PyAny> = native_field(refactor_project, "project_dir")?;
    let mut expansions: HashMap<String, Bound<'_, PyAny>> = HashMap::new();
    let items: Vec<Bound<'_, PyAny>> = native_field(&project, "sql_expansions")?
        .call_method0("items")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    for item in items.iter().rev() {
        let (path, expansion): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item.extract()?;
        expansions.insert(path.call_method0("resolve")?.str()?.extract()?, expansion);
    }
    let mut facts: Vec<ModelFacts> = Vec::new();
    for compiled in native_field(&project, "models")?.try_iter()? {
        let compiled: Bound<'_, PyAny> = compiled?;
        let resolved: String = project_dir
            .div(native_field(&compiled, "relative_path")?)?
            .call_method0("resolve")?
            .str()?
            .extract()?;
        facts.push(model(&compiled, expansions.get(&resolved))?);
    }
    Ok(Some(facts))
}
