//! A new view of the process environment as compile text reads it.

use crate::model_config::_helpers::model_nodes::unicode_environment::UnicodeEnvironment;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

/// A new view of the process environment as compile text reads it.
pub fn unicode_environment(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    Ok(Bound::new(py, UnicodeEnvironment)?.into_any())
}
