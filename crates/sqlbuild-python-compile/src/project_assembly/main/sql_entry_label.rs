//! How errors name a SQL hook entry: `{prefix} sql("name")`, or `{prefix} inline_sql("...")`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// How errors name a SQL hook entry: `{prefix} sql("name")`, or `{prefix} inline_sql("...")`.
pub fn sql_entry_label(prefix: &str, entry: &Bound<'_, PyAny>) -> PyResult<String> {
    let name: Bound<'_, PyAny> = native_field(entry, "name")?;
    Ok(if name.is_none() {
        format!("{prefix} inline_sql(\"...\")")
    } else {
        format!("{prefix} sql(\"{}\")", name.str()?)
    })
}
