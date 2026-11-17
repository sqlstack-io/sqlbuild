//! Microbatch events, scopes and intervals read from their Python shapes.

use sqlbuild_python_base::native_host::main::host_error::host_error;
use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_cursor::algebra::main::cursor_sort_key::cursor_sort_key;
use sqlbuild_cursor::algebra::models::{Scalar, SortKey};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_state::microbatches::models::{BoundInterval, CursorBound};

use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::cursor::main::scalars::as_scalar::as_scalar;
use sqlbuild_python_front::cursor::models::CursorSession;

/// `MicrobatchStateError(message)`.
pub(crate) fn state_error(py: Python<'_>, message: &str) -> PyErr {
    host_error(py, ExecutorHost::MicrobatchStateError, message)
}

/// `str(value)` of a string enum member or string.
pub(crate) fn enum_text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.str()?.extract()
}

/// `None`, or the string an optional field holds.
pub(crate) fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.extract()?))
}

/// Raw cursor bounds of one cursor type, parsed once per distinct text.
pub(crate) struct BoundParser<'py> {
    session: CursorSession<'py>,
    cursor_type: Bound<'py, PyAny>,
    keys: HashMap<String, SortKey>,
}

impl<'py> BoundParser<'py> {
    pub(crate) fn new(cursor_type: &Bound<'py, PyAny>) -> PyResult<Self> {
        Ok(Self {
            session: CursorSession::new(cursor_type.py())?,
            cursor_type: cursor_type.clone(),
            keys: HashMap::new(),
        })
    }

    /// One raw bound and its sort key.
    pub(crate) fn bound(&mut self, raw: &Bound<'py, PyAny>) -> PyResult<CursorBound> {
        let text: String = raw.extract()?;
        if let Some(key) = self.keys.get(&text) {
            return Ok(CursorBound { text, key: *key });
        }
        let py: Python<'py> = raw.py();
        let scalar: Scalar = as_scalar(
            &self.session,
            &PyString::new(py, &text).into_any(),
            &self.cursor_type,
        )?;
        let key: SortKey = self
            .session
            .checked(cursor_sort_key(&scalar, &self.session))?;
        self.keys.insert(text.clone(), key);
        Ok(CursorBound { text, key })
    }

    /// The bounds of an object with `start` and `end` attributes.
    pub(crate) fn interval(&mut self, interval: &Bound<'py, PyAny>) -> PyResult<BoundInterval> {
        Ok(BoundInterval {
            start: self.bound(&native_field(interval, "start")?)?,
            end: self.bound(&native_field(interval, "end")?)?,
        })
    }
}
