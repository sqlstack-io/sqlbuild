//! Public records of authored values.

use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::types::RecordId;

/// The Python classes authored values are told apart by and normalized values are built from.
#[derive(Debug)]
pub struct SqlValueClasses<'py> {
    pub(crate) decimal: Bound<'py, PyAny>,
    pub(crate) authored_set: Bound<'py, PyAny>,
    pub(crate) value_call: Bound<'py, PyAny>,
    pub(crate) sql_value: Bound<'py, PyAny>,
    pub(crate) logical_type: Bound<'py, PyAny>,
    pub(crate) kind: Bound<'py, PyAny>,
}

impl<'py> SqlValueClasses<'py> {
    pub fn load(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            decimal: py.import("decimal")?.getattr("Decimal")?,
            authored_set: record_class(py, RecordId::AuthoredSqlSet)?,
            value_call: record_class(py, RecordId::AuthoredSqlValueCall)?,
            sql_value: record_class(py, RecordId::SqlValue)?,
            logical_type: record_class(py, RecordId::SqlLogicalType)?,
            kind: record_class(py, RecordId::SqlValueKind)?,
        })
    }
}
