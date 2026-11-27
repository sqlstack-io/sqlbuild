//! A SQLBuild warehouse connection as SQLLoad's caller connection, and one session on it.

use std::path::Path;
use std::sync::Arc;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBytes, PyDict, PyDictMethods};
use sqlload::adapters::contract::models::TableName;
use sqlload::adapters::session::errors::SessionError;
use sqlload::adapters::session::types::{CallerConnection, CallerSession};

use crate::sessions::_helpers::failures::session_error;
use crate::sessions::constants::{
    CLOSE, COPY_IN, DATA, EXECUTE, FILE, LOAD_PARQUET, QUERY, QUERY_TEXT, STATEMENT, TABLE,
};

/// A target's warehouse connection: `opener` returns a new Python `LoadSession` each time SQLLoad opens one.
#[derive(Debug)]
pub struct PythonConnection {
    pub opener: Py<PyAny>,
    /// What the connection connects as, for SQLLoad's destination check line.
    pub description: String,
}

/// One Python `LoadSession`, closed on a thread of its own when SQLLoad drops it.
#[derive(Debug)]
pub struct PythonSession {
    pub session: Option<Py<PyAny>>,
}

impl CallerConnection for PythonConnection {
    fn open(&self) -> Result<Arc<dyn CallerSession>, SessionError> {
        Python::attach(|py| {
            let session: Py<PyAny> = self
                .opener
                .bind(py)
                .call0()
                .map_err(|error| session_error(py, &error))?
                .unbind();
            Ok(Arc::new(PythonSession {
                session: Some(session),
            }) as Arc<dyn CallerSession>)
        })
    }

    fn describe(&self) -> String {
        self.description.clone()
    }
}

impl PythonSession {
    /// `method` called with `keywords`, its answer read by `read`, a Python exception as SQLLoad's session error.
    fn call<T>(
        &self,
        method: &str,
        keywords: impl for<'py> FnOnce(&Bound<'py, PyDict>) -> PyResult<()>,
        read: impl for<'py> FnOnce(&Bound<'py, PyAny>) -> PyResult<T>,
    ) -> Result<T, SessionError> {
        let Some(session) = &self.session else {
            return Err(SessionError::new("the load session is closed"));
        };
        Python::attach(|py| {
            let arguments: Bound<'_, PyDict> = PyDict::new(py);
            keywords(&arguments)
                .and_then(|()| session.bind(py).call_method(method, (), Some(&arguments)))
                .and_then(|answer| read(&answer))
                .map_err(|error| session_error(py, &error))
        })
    }
}

impl CallerSession for PythonSession {
    fn execute(&self, statement: &str) -> Result<(), SessionError> {
        self.call(
            EXECUTE,
            |keywords| keywords.set_item(STATEMENT, statement),
            |_| Ok(()),
        )
    }

    fn query_text(&self, query: &str) -> Result<Vec<Vec<Option<String>>>, SessionError> {
        self.call(
            QUERY_TEXT,
            |keywords| keywords.set_item(QUERY, query),
            |answer| answer.extract(),
        )
    }

    fn copy_in(&self, statement: &str, data: &[u8]) -> Result<(), SessionError> {
        self.call(
            COPY_IN,
            |keywords| {
                keywords.set_item(STATEMENT, statement)?;
                keywords.set_item(DATA, PyBytes::new(keywords.py(), data))
            },
            |_| Ok(()),
        )
    }

    fn load_parquet(&self, table: &TableName, file: &Path) -> Result<(), SessionError> {
        let name = (
            table.database.clone(),
            table.schema.clone(),
            table.table.clone(),
        );
        let file = file.to_string_lossy().into_owned();
        self.call(
            LOAD_PARQUET,
            |keywords| {
                keywords.set_item(TABLE, name)?;
                keywords.set_item(FILE, file)
            },
            |_| Ok(()),
        )
    }
}

impl Drop for PythonSession {
    fn drop(&mut self) {
        let Some(session) = self.session.take() else {
            return;
        };
        let closing = std::thread::Builder::new()
            .name("sqlbuild-load-session-close".to_owned())
            .spawn(move || {
                Python::attach(|py| {
                    let _closed = session.bind(py).call_method0(CLOSE);
                });
            });
        drop(closing);
    }
}
