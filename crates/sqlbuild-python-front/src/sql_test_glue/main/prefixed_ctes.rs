//! Extract fixture bodies from authored test or scenario CTEs in authored order.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Extract fixture bodies from authored test or scenario CTEs in authored order.
pub fn prefixed_ctes<'py>(ctes: &Bound<'py, PyAny>, prefix: &str) -> PyResult<Bound<'py, PyDict>> {
    let result = PyDict::new(ctes.py());
    for cte in ctes.try_iter()? {
        let cte = cte?;
        let name: String = native_field(&cte, "name")?.extract()?;
        if let Some(name) = name.strip_prefix(prefix) {
            result.set_item(name, native_field(&cte, "sql_body")?)?;
        }
    }
    Ok(result)
}
