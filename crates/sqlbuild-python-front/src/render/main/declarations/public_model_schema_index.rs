//! Every public model schema by name, with its inherited columns first, collision-checked.

use crate::render::_helpers::declarations::declaration_indexes::SchemaResolution;
use crate::render::_helpers::declarations::declaration_indexes::compile_input_error;
use crate::render::_helpers::declarations::declaration_indexes::text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::collections::HashMap;

/// Every public model schema by name, with its inherited columns first, collision-checked.
#[pyfunction]
pub fn public_model_schema_index<'py>(inputs: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = inputs.py();
    let mut authored: Vec<(String, Bound<'py, PyAny>)> = Vec::new();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for file in native_field(&inputs, "model_schema_files")?.try_iter()? {
        for declaration in native_field(&file?, "declarations")?.try_iter()? {
            let declaration: Bound<'py, PyAny> = declaration?;
            let name: String = native_field(&declaration, "name")?.extract()?;
            if let Some(index) = by_name.get(&name) {
                return Err(compile_input_error(
                    py,
                    format!(
                        "Duplicate public schema '{name}' in {} and {}",
                        text(&authored[*index].1, "relative_path")?,
                        text(&declaration, "relative_path")?
                    ),
                ));
            }
            by_name.insert(name.clone(), authored.len());
            authored.push((name, declaration));
        }
    }
    let names: Vec<String> = authored.iter().map(|(name, _)| name.clone()).collect();
    let mut resolution: SchemaResolution<'py> = SchemaResolution {
        py,
        authored,
        by_name,
        resolved: HashMap::new(),
        order: Vec::new(),
    };
    let index: Bound<'py, PyDict> = PyDict::new(py);
    for name in &names {
        resolution.resolve(name, &[])?;
    }
    for name in &resolution.order {
        index.set_item(name, &resolution.resolved[name])?;
    }
    Ok(index)
}
