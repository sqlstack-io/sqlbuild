//! Direct reads of native pyclass `#[getter]` attributes from installed per-module tables.

use std::collections::HashMap;
use std::sync::OnceLock;

use pyo3::PyTypeInfo;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::PyType;

use crate::records::models::GetterEntry;
use crate::records::types::GetterRead;

static TABLES: OnceLock<&'static [&'static [GetterEntry]]> = OnceLock::new();
static DISPATCH: PyOnceLock<HashMap<usize, GetterRead>> = PyOnceLock::new();

/// The type object of the native pyclass `T`, as a getter table entry names it.
pub(crate) fn class_of<'py, T: PyTypeInfo>(py: Python<'py>) -> Bound<'py, PyType> {
    py.get_type::<T>()
}

/// Install every module's `getters::GETTERS` table once, before any record field is read.
pub(crate) fn install(tables: &'static [&'static [GetterEntry]]) {
    let _ = TABLES.get_or_init(|| tables);
}

/// The `#[getter]` `name` of a native pyclass instance read in Rust, or `None` when not generated here.
pub(crate) fn getter_field<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let tables: &'static [&'static [GetterEntry]] = TABLES.get().copied()?;
    let py: Python<'py> = value.py();
    let table: &HashMap<usize, GetterRead> = DISPATCH.get_or_init(py, || {
        let mut table: HashMap<usize, GetterRead> = HashMap::new();
        for entries in tables {
            for entry in entries.iter() {
                let _ = table.insert((entry.class)(py).as_ptr() as usize, entry.read);
            }
        }
        table
    });
    let read: &GetterRead = table.get(&(value.get_type().as_ptr() as usize))?;
    read(value, name)
}
