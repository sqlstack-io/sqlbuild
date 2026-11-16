//! A source as the table factor that reads it, deduplicated with `FINAL` when `final`.

use crate::references::_helpers::source_relations::FINAL_READ_DIALECT;
use crate::references::_helpers::source_relations::final_unsupported;
use crate::references::main::render_source_relation::render_source_relation;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// A source as the table factor that reads it, deduplicated with `FINAL` when `final`.
pub fn render_source_read<'py>(
    entry: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<String> {
    let relation: String = render_source_relation(entry, Some(adapter))?;
    if !native_field(entry, "final")?.is_truthy()? {
        return Ok(relation);
    }
    let dialect: Option<String> = native_field(adapter, "sql_analysis_dialect_name")?.extract()?;
    if dialect.as_deref() != Some(FINAL_READ_DIALECT) {
        return Err(final_unsupported(
            entry.py(),
            &native_field(adapter, "adapter_name")?.str()?.to_string(),
        ));
    }
    Ok(format!("(SELECT * FROM {relation} FINAL)"))
}
