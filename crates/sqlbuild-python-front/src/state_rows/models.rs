//! Public records of state rows.

use pyo3::prelude::{Bound, PyAny};

/// Which migration state table a request reads or writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventTable {
    Model,
    Column,
    OldName,
}

/// Name, columns, event relation attribute and messages of one migration state table.
#[derive(Clone, Copy, Debug)]
pub struct EventTableLayout {
    pub(crate) table: EventTable,
    pub name: &'static str,
    pub columns: &'static [&'static str],
    pub relation_attribute: &'static str,
    pub state_message: &'static str,
    pub event_message: &'static str,
    pub read_subject: &'static str,
}

/// How migration state reaches the warehouse: the connection, execute callable and name renderer.
#[derive(Debug)]
pub struct StateAccess<'py> {
    pub connection: Bound<'py, PyAny>,
    pub execute: Bound<'py, PyAny>,
    pub render_qualified_name: Bound<'py, PyAny>,
}
