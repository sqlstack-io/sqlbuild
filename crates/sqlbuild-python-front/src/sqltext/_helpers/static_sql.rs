//! Interpolate `@@` tokens in authored SQL and extract static SQL references.

use crate::sqltext::models::InterpolationSources;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;
use std::cell::RefCell;

use pyo3::PyErr;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods};
use pyo3::types::{PyDict, PyDictMethods, PyListMethods, PyString, PyStringMethods};
use sqlbuild_sqltext::compiler::models::InterpolationRead;
use sqlbuild_sqltext::compiler::types::InterpolationHost;

const ENVIRONMENT_READ: &str = "env";
const CONTEXT_READ: &str = "ctx";

pub(crate) struct PythonInterpolationHost<'py> {
    pub(crate) sources: InterpolationSources<'py>,
    /// A Python exception other than a rendering `ValueError`, raised after the scan.
    pub(crate) raised: RefCell<Option<PyErr>>,
}

impl PythonInterpolationHost<'_> {
    fn hold(&self, error: PyErr) -> String {
        let message = error.to_string();
        self.raised.borrow_mut().get_or_insert(error);
        message
    }

    fn text(&self, value: &Bound<'_, PyAny>) -> Result<String, String> {
        value
            .cast::<PyString>()
            .map_err(|error| self.hold(error.into()))?
            .to_str()
            .map(str::to_owned)
            .map_err(|error| self.hold(error))
    }
}

impl InterpolationHost for PythonInterpolationHost<'_> {
    fn variable(&self, name: &str) -> Option<Result<String, String>> {
        let value = match self.sources.0.get_item(name) {
            Ok(value) => value?,
            Err(error) => return Some(Err(self.hold(error))),
        };
        if value.is_exact_instance_of::<PyString>() {
            return Some(self.text(&value));
        }
        let label = format!("SQL variable '@@{name}'");
        let py = value.py();
        let arguments = PyDict::new(py);
        if let Err(error) = arguments
            .set_item("value", value)
            .and_then(|()| arguments.set_item("label", label))
        {
            return Some(Err(self.hold(error)));
        }
        Some(match self.sources.3.call((), Some(&arguments)) {
            Ok(rendered) => self.text(&rendered),
            Err(error) if error.is_instance_of::<PyValueError>(py) => {
                Err(error.value(py).to_string())
            }
            Err(error) => Err(self.hold(error)),
        })
    }

    fn variable_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .sources
            .0
            .keys()
            .iter()
            .map(|key| key_text(&key))
            .collect();
        names.sort();
        names
    }

    fn environment(&self, name: &str) -> Result<Option<String>, String> {
        let environment = &self.sources.1;
        if !environment
            .contains(name)
            .map_err(|error| self.hold(error))?
        {
            return Ok(None);
        }
        let value = environment
            .get_item(name)
            .map_err(|error| self.hold(error))?;
        self.text(&value).map(Some)
    }

    fn context_allowed(&self) -> bool {
        self.sources.2.is_some()
    }

    fn context(&self, name: &str) -> Option<Option<String>> {
        let value = match self.sources.2.as_ref()?.get_item(name) {
            Ok(value) => value?,
            Err(error) => {
                let _ = self.hold(error);
                return None;
            }
        };
        if value.is_none() {
            return Some(None);
        }
        match self.text(&value) {
            Ok(text) => Some(Some(text)),
            Err(_held) => Some(None),
        }
    }

    fn lexical_syntax(&self) -> Option<&LexicalSyntax> {
        self.sources.4.as_ref()
    }

    fn context_names(&self) -> Vec<String> {
        self.sources
            .2
            .iter()
            .flat_map(pyo3::types::PyDictMethods::keys)
            .map(|key| key_text(&key))
            .collect()
    }
}

/// A variable or context key's text, or empty text when Python cannot render it.
fn key_text(key: &Bound<'_, PyAny>) -> String {
    key.str().map(|text| text.to_string()).unwrap_or_default()
}

/// `(sql or None when unchanged, reads, error message or None)` for one SQL text.
pub(crate) type InterpolatedRow = (Option<String>, Vec<(&'static str, String)>, Option<String>);

pub(crate) fn read_rows(reads: Vec<InterpolationRead>) -> Vec<(&'static str, String)> {
    reads
        .into_iter()
        .map(|read| match read {
            InterpolationRead::Environment(name) => (ENVIRONMENT_READ, name),
            InterpolationRead::Context(name) => (CONTEXT_READ, name),
        })
        .collect()
}
