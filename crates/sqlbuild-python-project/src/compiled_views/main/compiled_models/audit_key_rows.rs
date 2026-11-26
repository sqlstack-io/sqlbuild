//! Each audit spec's `(definition name, column argument when it is text)`.

use crate::compiled_views::_helpers::compiled_models::relation_keys::UNIQUE_AUDIT_COLUMN_ARGUMENT;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Each audit spec's `(definition name, column argument when it is text)`.
pub fn audit_key_rows(audits: &Bound<'_, PyAny>) -> PyResult<Vec<(String, Option<String>)>> {
    let mut rows: Vec<(String, Option<String>)> = Vec::new();
    for audit in audits.try_iter()? {
        let audit: Bound<'_, PyAny> = audit?;
        let argument: Option<Bound<'_, PyAny>> = native_field(&audit, "arguments")?
            .cast::<PyDict>()?
            .get_item(UNIQUE_AUDIT_COLUMN_ARGUMENT)?;
        rows.push((
            native_field(&audit, "definition_name")?.extract()?,
            argument
                .filter(pyo3::types::PyAnyMethods::is_instance_of::<PyString>)
                .map(|value| value.extract())
                .transpose()?,
        ));
    }
    Ok(rows)
}
