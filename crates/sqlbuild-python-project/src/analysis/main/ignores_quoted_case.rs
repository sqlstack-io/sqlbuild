//! `ignores_quoted_case`: whether the connection explicitly enables case-insensitive quoting.

use crate::analysis::_helpers::binding_catalog::QUOTED_CASE_PARAMETER;
use crate::analysis::_helpers::binding_catalog::SESSION_PARAMETERS;
use crate::analysis::_helpers::binding_catalog::SNOWFLAKE_DIALECT;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyDictMethods, PyResult};
use pyo3::types::{PyBool, PyDict};
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;

/// `ignores_quoted_case`: whether the connection explicitly enables case-insensitive quoting.
pub fn ignores_quoted_case(
    connection: &Bound<'_, PyAny>,
    dialect: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    if dialect.is_none() || dialect.extract::<String>()? != SNOWFLAKE_DIALECT {
        return Ok(false);
    }
    let parameters: Bound<'_, PyAny> = mapping_get(connection, SESSION_PARAMETERS)?;
    let Ok(parameters) = parameters.downcast::<PyDict>() else {
        return Ok(false);
    };
    for (key, value) in parameters.iter() {
        let upper: Bound<'_, PyAny> = key.str()?.call_method0("upper")?;
        if upper.eq(QUOTED_CASE_PARAMETER)? && value.is(PyBool::new(key.py(), true)) {
            return Ok(true);
        }
    }
    Ok(false)
}
