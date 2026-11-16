//! The public declaration indexes: enums, constants and resolved reusable model schemas.

use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use std::collections::HashMap;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::ErrorId;

/// `CompileInputError(message)`.
fn compile_input_error(py: Python<'_>, message: String) -> PyErr {
    new_plain_error(py, ErrorId::CompileInput, &message)
}

fn text(owner: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    Ok(native_field(owner, name)?.str()?.to_string())
}

/// Add each declaration of `files` to `index`, rejecting a name declared twice.
fn index_declarations<'py>(
    files: &Bound<'py, PyAny>,
    kind: &str,
    index: &Bound<'py, PyDict>,
) -> PyResult<()> {
    let py: Python<'py> = files.py();
    for file in files.try_iter()? {
        for declaration in native_field(&file?, "declarations")?.try_iter()? {
            let declaration: Bound<'py, PyAny> = declaration?;
            let name: Bound<'py, PyAny> = native_field(&declaration, "name")?;
            if let Some(existing) = index.get_item(&name)? {
                return Err(compile_input_error(
                    py,
                    format!(
                        "Duplicate public {kind} '{}' in {} and {}",
                        name.str()?,
                        text(&existing, "relative_path")?,
                        text(&declaration, "relative_path")?
                    ),
                ));
            }
            index.set_item(name, declaration)?;
        }
    }
    Ok(())
}

/// Every public enum and constant by name, collision-checked.
#[pyfunction]
pub(crate) fn public_declaration_indexes<'py>(
    inputs: Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyDict>)> {
    let py: Python<'py> = inputs.py();
    let enums: Bound<'py, PyDict> = PyDict::new(py);
    let constants: Bound<'py, PyDict> = PyDict::new(py);
    index_declarations(&native_field(&inputs, "enum_files")?, "enum", &enums)?;
    index_declarations(
        &native_field(&inputs, "constant_files")?,
        "constant",
        &constants,
    )?;
    Ok((enums, constants))
}

/// Resolving `extends` chains across the authored public model schemas.
struct SchemaResolution<'py> {
    py: Python<'py>,
    authored: Vec<(String, Bound<'py, PyAny>)>,
    by_name: HashMap<String, usize>,
    resolved: HashMap<String, Bound<'py, PyAny>>,
    /// Names in the order Python's `resolved` dict gains them.
    order: Vec<String>,
}

impl<'py> SchemaResolution<'py> {
    /// Python's `_resolve_model_schema_declaration`.
    fn resolve(&mut self, name: &str, resolving: &[String]) -> PyResult<Bound<'py, PyAny>> {
        if let Some(existing) = self.resolved.get(name) {
            return Ok(existing.clone());
        }
        if let Some(start) = resolving.iter().position(|item| item == name) {
            let mut cycle: Vec<&str> = resolving[start..].iter().map(String::as_str).collect();
            cycle.push(name);
            return Err(compile_input_error(
                self.py,
                format!("Model schema inheritance cycle: {}", cycle.join(" -> ")),
            ));
        }
        let declaration: Bound<'py, PyAny> = self.authored[self.by_name[name]].1.clone();
        let extends: Option<String> = native_field(&declaration, "extends")?.extract()?;
        let mut inherited: Vec<Bound<'py, PyAny>> = Vec::new();
        if let Some(parent) = extends {
            if !self.by_name.contains_key(&parent) {
                return Err(compile_input_error(
                    self.py,
                    format!(
                        "Schema '{name}' in {} extends unknown schema '{parent}'",
                        text(&declaration, "relative_path")?
                    ),
                ));
            }
            let mut chain: Vec<String> = resolving.to_vec();
            chain.push(name.to_owned());
            let parent: Bound<'py, PyAny> = self.resolve(&parent, &chain)?;
            for column in native_field(&parent, "columns")?.try_iter()? {
                inherited.push(column?);
            }
        }
        self.reject_overrides(name, &declaration, &inherited)?;
        let mut columns: Vec<Bound<'py, PyAny>> = inherited;
        for column in native_field(&declaration, "columns")?.try_iter()? {
            columns.push(column?);
        }
        let fields: Bound<'py, PyDict> = PyDict::new(self.py);
        fields.set_item("columns", PyTuple::new(self.py, columns)?)?;
        let resolved: Bound<'py, PyAny> = replaced_keywords(&declaration, &fields)?;
        self.resolved.insert(name.to_owned(), resolved.clone());
        self.order.push(name.to_owned());
        Ok(resolved)
    }

    /// Python's `_validate_no_inherited_column_overrides`.
    fn reject_overrides(
        &self,
        name: &str,
        declaration: &Bound<'py, PyAny>,
        inherited: &[Bound<'py, PyAny>],
    ) -> PyResult<()> {
        let mut by_name: HashMap<String, &Bound<'py, PyAny>> = HashMap::new();
        for column in inherited {
            by_name.insert(lowered_name(column)?, column);
        }
        for local in native_field(declaration, "columns")?.try_iter()? {
            let local: Bound<'py, PyAny> = local?;
            let Some(column) = by_name.get(&lowered_name(&local)?) else {
                continue;
            };
            let location: Bound<'py, PyAny> = native_field(column, "location")?;
            let origin: String = if location.is_none() {
                "an ancestor schema".to_owned()
            } else {
                format!("{}:{}", text(&location, "path")?, text(&location, "line")?)
            };
            return Err(compile_input_error(
                self.py,
                format!(
                    "Schema '{name}' in {} redeclares inherited column '{}' from {origin}; \
                     column overrides are not supported",
                    text(declaration, "relative_path")?,
                    text(&local, "name")?
                ),
            ));
        }
        Ok(())
    }
}

/// `column.name.lower()`.
fn lowered_name(column: &Bound<'_, PyAny>) -> PyResult<String> {
    native_field(column, "name")?
        .call_method0("lower")?
        .extract()
}

/// Every public model schema by name, with its inherited columns first, collision-checked.
#[pyfunction]
pub(crate) fn public_model_schema_index<'py>(
    inputs: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(public_declaration_indexes, module)?)?;
    module.add_function(wrap_pyfunction!(public_model_schema_index, module)?)
}
