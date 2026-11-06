//! Interpolate `@@` tokens in authored SQL and extract static SQL references.

use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;
use std::cell::RefCell;

use pyo3::PyErr;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyListMethods, PyString, PyStringMethods};
use sqlbuild_sqltext::compiler::main::sql_interpolation::interpolate_sql;
use sqlbuild_sqltext::compiler::models::InterpolationRead;
use sqlbuild_sqltext::compiler::types::InterpolationHost;

use crate::bindings::_helpers::render::macro_call_rows::SpanRow;

const ENVIRONMENT_READ: &str = "env";
const CONTEXT_READ: &str = "ctx";

/// One SQL text interpolated: its SQL, substitution spans, reads and error message.
pub(crate) struct InterpolationRow {
    /// The interpolated SQL, or `None` when unchanged.
    pub(crate) sql: Option<String>,
    pub(crate) spans: Vec<SpanRow>,
    pub(crate) reads: Vec<(&'static str, String)>,
    pub(crate) error: Option<String>,
}

/// Variables, environment, context values (or `None`), variable renderer and lexical rules.
pub(crate) struct InterpolationSources<'py>(
    pub(crate) Bound<'py, PyDict>,
    pub(crate) Bound<'py, PyAny>,
    pub(crate) Option<Bound<'py, PyDict>>,
    pub(crate) Bound<'py, PyAny>,
    /// The lexical rules the SQL is authored in; `None` reads it under the generic rules.
    pub(crate) Option<LexicalSyntax>,
);

struct PythonInterpolationHost<'py> {
    sources: InterpolationSources<'py>,
    /// A Python exception other than a rendering `ValueError`, raised after the scan.
    raised: RefCell<Option<PyErr>>,
}

impl PythonInterpolationHost<'_> {
    fn hold(&self, error: PyErr) -> String {
        let message = error.to_string();
        self.raised.borrow_mut().get_or_insert(error);
        message
    }

    fn text(&self, value: &Bound<'_, PyAny>) -> Result<String, String> {
        value
            .downcast::<PyString>()
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

/// Each `(sql, file path)` interpolated in order; non-rendering Python failures raise.
pub(crate) fn interpolation_batch(
    sources: InterpolationSources<'_>,
    sqls: &[(String, String)],
) -> PyResult<Vec<InterpolationRow>> {
    let host = PythonInterpolationHost {
        sources,
        raised: RefCell::new(None),
    };
    let mut rows = Vec::with_capacity(sqls.len());
    for (sql, file_path) in sqls {
        let row = match interpolate_sql(&host, sql, file_path) {
            Ok(result) => InterpolationRow {
                sql: result.sql,
                spans: result.spans,
                reads: read_rows(result.reads),
                error: None,
            },
            Err(failure) => InterpolationRow {
                sql: None,
                spans: Vec::new(),
                reads: read_rows(failure.reads),
                error: Some(failure.message),
            },
        };
        if let Some(error) = host.raised.borrow_mut().take() {
            return Err(error);
        }
        rows.push(row);
    }
    Ok(rows)
}

/// `(sql or None when unchanged, reads, error message or None)` for one SQL text.
pub(crate) type InterpolatedRow = (Option<String>, Vec<(&'static str, String)>, Option<String>);

/// One SQL text interpolated: the SQL (`None` when unchanged), its reads, and its error.
pub(crate) fn interpolated(
    sources: InterpolationSources<'_>,
    sql: &str,
    file_path: &str,
) -> PyResult<InterpolatedRow> {
    let host = PythonInterpolationHost {
        sources,
        raised: RefCell::new(None),
    };
    let row = match interpolate_sql(&host, sql, file_path) {
        Ok(result) => (result.sql, read_rows(result.reads), None),
        Err(failure) => (None, read_rows(failure.reads), Some(failure.message)),
    };
    if let Some(error) = host.raised.borrow_mut().take() {
        return Err(error);
    }
    Ok(row)
}

fn read_rows(reads: Vec<InterpolationRead>) -> Vec<(&'static str, String)> {
    reads
        .into_iter()
        .map(|read| match read {
            InterpolationRead::Environment(name) => (ENVIRONMENT_READ, name),
            InterpolationRead::Context(name) => (CONTEXT_READ, name),
        })
        .collect()
}
