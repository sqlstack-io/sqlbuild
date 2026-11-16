//! The project relation `relation` names, resolving missing qualifiers from the defaults.

use crate::references::_helpers::relations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The project relation `relation` names, resolving missing qualifiers from the defaults.
#[inline]
pub fn match_project_relation<'py>(
    index: &Bound<'py, PyAny>,
    relation: &Bound<'py, PyAny>,
    default_database: Option<String>,
    default_schema: Option<String>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    relations::match_project_relation(index, relation, default_database, default_schema)
}
