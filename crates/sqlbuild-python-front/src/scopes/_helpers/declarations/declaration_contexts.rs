//! Declaration resolution contexts built natively from a Python scope lookup's positions.

use std::collections::HashMap;

use pyo3::FromPyObject;
use pyo3::prelude::{Bound, Py, PyAny};
use pyo3::types::PyDict;

use sqlbuild_scopes::scope_index::models::VisibleEntry;

pub(crate) const ENUM_KIND: u8 = 0;
pub(crate) const CONSTANT_KIND: u8 = 1;
pub(crate) const MACRO_KIND: u8 = 2;

pub(crate) type TableRow = (
    Vec<usize>,
    HashMap<String, Vec<usize>>,
    HashMap<String, Vec<usize>>,
);
pub(crate) type VisibleRow<'py> = (Bound<'py, PyAny>, VisibleEntry, Option<Bound<'py, PyAny>>);

/// One declaration position: identity, first record, name, kinds, runtime value and key.
#[derive(Debug, FromPyObject)]
pub(crate) struct PositionRow(
    pub(crate) Py<PyAny>,
    pub(crate) Py<PyAny>,
    pub(crate) Py<PyAny>,
    pub(crate) u8,
    pub(crate) Py<PyAny>,
    pub(crate) u8,
    pub(crate) u32,
);

/// The Python classes and enum members the contexts are built from.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct ContextClasses {
    pub(crate) visibility_record: Py<PyAny>,
    pub(crate) resolution_context: Py<PyAny>,
    pub(crate) reasons: Py<PyDict>,
}

/// The Python lookup mappings a matching resource's private positions and grants come from.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct LookupMappings {
    pub(crate) identity_keys: Py<PyDict>,
    pub(crate) private_positions: Py<PyAny>,
    pub(crate) grants_by_resource: Py<PyAny>,
}

/// Dictionaries one context is assembled into, in Python's insertion order.
pub(crate) struct ContextParts<'py> {
    pub(crate) enums: Bound<'py, PyDict>,
    pub(crate) constants: Bound<'py, PyDict>,
    pub(crate) macros: Bound<'py, PyDict>,
    pub(crate) macro_records: Bound<'py, PyDict>,
    pub(crate) inaccessible: [Bound<'py, PyDict>; 3],
    pub(crate) visibility: [Bound<'py, PyDict>; 3],
}
