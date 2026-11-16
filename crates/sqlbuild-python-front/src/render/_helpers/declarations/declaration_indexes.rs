//! The public declaration indexes: enums, constants and resolved reusable model schemas.

use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::ErrorId;

/// `CompileInputError(message)`.
pub(crate) fn compile_input_error(py: Python<'_>, message: String) -> PyErr {
    new_plain_error(py, ErrorId::CompileInput, &message)
}

pub(crate) fn text(owner: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    Ok(native_field(owner, name)?.str()?.to_string())
}

/// Add each declaration of `files` to `index`, rejecting a name declared twice.
pub(crate) fn index_declarations<'py>(
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

/// Resolving `extends` chains across the authored public model schemas.
pub(crate) struct SchemaResolution<'py> {
    pub(crate) py: Python<'py>,
    pub(crate) authored: Vec<(String, Bound<'py, PyAny>)>,
    pub(crate) by_name: HashMap<String, usize>,
    pub(crate) resolved: HashMap<String, Bound<'py, PyAny>>,
    /// Names in the order Python's `resolved` dict gains them.
    pub(crate) order: Vec<String>,
}

impl<'py> SchemaResolution<'py> {
    /// Python's `_resolve_model_schema_declaration`.
    pub(crate) fn resolve(
        &mut self,
        name: &str,
        resolving: &[String],
    ) -> PyResult<Bound<'py, PyAny>> {
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
